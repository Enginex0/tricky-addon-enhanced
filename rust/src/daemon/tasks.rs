use std::path::Path;

use crate::config::Config;
use crate::platform::network::wait_for_network;

const DATA_DIR: &str = "/data/adb/tricky_store/ta-enhanced";

pub struct TaskBackoff(pub u32);

pub trait DaemonTask {
    fn name(&self) -> &'static str;
    fn is_enabled(&self, config: &Config) -> bool;
    fn interval_secs(&self, config: &Config) -> u32;
    fn run(&mut self, config: &Config, manager: Option<&str>) -> Result<(), TaskBackoff>;
}

fn is_screen_off() -> bool {
    std::process::Command::new("dumpsys")
        .arg("power")
        .output()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            s.contains("mWakefulness=Asleep") || s.contains("state=OFF")
        })
        .unwrap_or(false)
}

fn is_uninstall_pending(module_id: &str) -> bool {
    let markers = [
        format!("/data/adb/modules/.{module_id}"),
        format!("/data/adb/modules/{module_id}/remove"),
    ];
    markers.iter().any(|m| Path::new(m).exists())
}

// --- StatusTask ---

pub struct StatusTask {
    last_description: String,
    original_saved: bool,
}

impl StatusTask {
    pub fn new() -> Self {
        Self {
            last_description: String::new(),
            original_saved: false,
        }
    }
}

impl DaemonTask for StatusTask {
    fn name(&self) -> &'static str { "status" }
    fn is_enabled(&self, config: &Config) -> bool { config.status.enabled }
    fn interval_secs(&self, config: &Config) -> u32 { config.status.interval }

    fn run(&mut self, config: &Config, _manager: Option<&str>) -> Result<(), TaskBackoff> {
        if !self.original_saved {
            let _ = crate::status::save_original_description();
            self.original_saved = true;
        }

        if is_uninstall_pending(&config.general.module_id) {
            let _ = crate::status::restore_original_description();
            return Ok(());
        }

        let desc = crate::status::build_description(config);
        if desc != self.last_description {
            if let Err(e) = crate::status::update_prop_description(&desc) {
                tracing::warn!("status update failed: {e}");
                return Ok(());
            }
            tracing::info!("status: {desc}");
            self.last_description = desc;
        }

        Ok(())
    }
}

// --- AutomationTask ---

pub struct AutomationTask;

impl AutomationTask {
    pub fn new() -> Self { Self }
}

impl DaemonTask for AutomationTask {
    fn name(&self) -> &'static str { "automation" }
    fn is_enabled(&self, config: &Config) -> bool { config.automation.enabled }
    fn interval_secs(&self, config: &Config) -> u32 { config.automation.interval }

    fn run(&mut self, config: &Config, manager: Option<&str>) -> Result<(), TaskBackoff> {
        let pending = Path::new(DATA_DIR).join("applist.pending");
        if pending.exists() {
            match std::fs::read_to_string(&pending) {
                Ok(content) => {
                    let entries: Vec<String> = content.lines().map(str::to_owned).collect();
                    if let Err(e) = crate::automation::target::write_target(&entries) {
                        tracing::warn!("applist.pending apply failed: {e}");
                    } else {
                        let _ = std::fs::remove_file(&pending);
                        let count = content.lines().filter(|l| !l.is_empty()).count();
                        tracing::info!("applied applist.pending ({count} entries)");
                    }
                }
                Err(e) => tracing::warn!("applist.pending read failed: {e}"),
            }
        }

        if let Err(e) = crate::automation::watcher::check_new_packages(
            &config.automation.exclude_list,
            manager,
        ) {
            tracing::warn!("package check failed: {e}");
        }
        if let Err(e) = crate::automation::watcher::cleanup_dead_apps() {
            tracing::warn!("dead app cleanup failed: {e}");
        }
        Ok(())
    }
}

// --- HealthTask ---

pub struct HealthTask {
    state: crate::health::HealthState,
}

impl HealthTask {
    pub fn new() -> Self {
        Self {
            state: crate::health::HealthState::default(),
        }
    }
}

impl DaemonTask for HealthTask {
    fn name(&self) -> &'static str { "health" }
    fn is_enabled(&self, config: &Config) -> bool { config.health.enabled }
    fn interval_secs(&self, config: &Config) -> u32 { config.health.interval }

    fn run(&mut self, config: &Config, _manager: Option<&str>) -> Result<(), TaskBackoff> {
        match crate::health::check_once(&mut self.state, config) {
            Ok(healthy) => {
                if !healthy && self.state.restarts > 0 {
                    let backoff = self.state.backoff_secs.max(config.health.backoff_init as u64);
                    return Err(TaskBackoff(backoff as u32));
                }
                Ok(())
            }
            Err(e) => {
                tracing::warn!("health check error: {e}");
                Ok(())
            }
        }
    }
}

// --- KeyboxTask ---

pub struct KeyboxTask {
    boot_done: bool,
}

impl KeyboxTask {
    pub fn new() -> Self {
        Self { boot_done: false }
    }
}

impl DaemonTask for KeyboxTask {
    fn name(&self) -> &'static str { "keybox" }
    fn is_enabled(&self, config: &Config) -> bool { config.keybox.enabled }
    fn interval_secs(&self, config: &Config) -> u32 { config.keybox.interval }

    fn run(&mut self, config: &Config, _manager: Option<&str>) -> Result<(), TaskBackoff> {
        if !self.boot_done {
            if !wait_for_network(7) {
                if crate::engine::Engine::detect()
                    .keybox_path()
                    .map(|path| path.exists())
                    .unwrap_or(false)
                {
                    tracing::info!("no network at boot, keeping existing keybox");
                    self.boot_done = true;
                    return Ok(());
                }
                tracing::warn!("no network, no keybox -- retry next interval");
                self.boot_done = true;
                return Ok(());
            }

            for attempt in 1..=config.keybox.boot_retries {
                match crate::keybox::fetch(config) {
                    Ok(r) => {
                        tracing::info!("boot keybox fetch ok (attempt {attempt}, source={})", r.source);
                        self.boot_done = true;
                        return Ok(());
                    }
                    Err(e) => {
                        tracing::warn!("boot keybox attempt {attempt} failed: {e}");
                        if attempt < config.keybox.boot_retries {
                            std::thread::sleep(std::time::Duration::from_secs(config.keybox.retry_delay as u64));
                        }
                    }
                }
            }
            tracing::error!("all {} boot keybox attempts failed", config.keybox.boot_retries);
            self.boot_done = true;
            return Ok(());
        }

        if is_screen_off() {
            return Ok(());
        }

        if let Err(e) = crate::keybox::fetch(config) {
            tracing::warn!("keybox refresh failed: {e}");
        }
        Ok(())
    }
}

// --- SecurityPatchTask ---

pub struct SecurityPatchTask {
    boot_done: bool,
}

impl SecurityPatchTask {
    pub fn new() -> Self {
        Self { boot_done: false }
    }
}

impl DaemonTask for SecurityPatchTask {
    fn name(&self) -> &'static str { "security_patch" }
    fn is_enabled(&self, config: &Config) -> bool { config.security_patch.auto_update }
    fn interval_secs(&self, config: &Config) -> u32 { config.security_patch.interval }

    fn run(&mut self, config: &Config, _manager: Option<&str>) -> Result<(), TaskBackoff> {
        if !self.boot_done {
            if wait_for_network(7) {
                for attempt in 1..=config.security_patch.boot_retries {
                    match crate::security_patch::update(config) {
                        Ok(()) => {
                            tracing::info!("boot security patch fetch ok (attempt {attempt})");
                            self.boot_done = true;
                            return Ok(());
                        }
                        Err(e) => {
                            tracing::warn!("boot secpatch attempt {attempt} failed: {e}");
                            if attempt < config.security_patch.boot_retries {
                                std::thread::sleep(std::time::Duration::from_secs(3));
                            }
                        }
                    }
                }
            }
            tracing::warn!("boot security patch fetch failed -- retry next interval");
            self.boot_done = true;
            return Ok(());
        }

        if is_screen_off() {
            return Ok(());
        }

        if let Err(e) = crate::security_patch::update(config) {
            tracing::warn!("security patch update failed: {e}");
        }
        Ok(())
    }
}
