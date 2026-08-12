use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use crate::config::Config;
use crate::cli::HealthAction;
use crate::platform::fs::atomic_write;

const TS_MODULE: &str = "/data/adb/modules/tricky_store";
const TS_MODULE_HIDDEN: &str = "/data/adb/modules/.tricky_store";
const HEALTH_STATE: &str = "/data/adb/tricky_store/.health_state";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineStatus {
    Running,
    Restarting,
    Restarted,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CircuitState {
    #[default]
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthState {
    pub status: EngineStatus,
    pub pid: Option<u32>,
    pub restarts: u32,
    pub engine: String,
    pub last_check: u64,
    pub last_restart: u64,
    #[serde(default)]
    pub circuit: CircuitState,
    #[serde(default)]
    pub backoff_secs: u64,
}

impl Default for HealthState {
    fn default() -> Self {
        Self {
            status: EngineStatus::Unknown,
            pid: None,
            restarts: 0,
            engine: detect_engine(),
            last_check: 0,
            last_restart: 0,
            circuit: CircuitState::Closed,
            backoff_secs: 0,
        }
    }
}

pub fn handle_health(action: HealthAction, cfg: &Config) -> anyhow::Result<()> {
    if !cfg.health.enabled {
        println!("health monitoring disabled");
        return Ok(());
    }

    match action {
        HealthAction::TeeStatus => {
            let state = tee_status()?;
            println!("{}", serde_json::to_string_pretty(&state)?);
            Ok(())
        }
        HealthAction::Status => {
            let state = read_state().unwrap_or_default();
            println!("{}", serde_json::to_string_pretty(&state)?);
            Ok(())
        }
    }
}

pub fn detect_engine() -> String {
    if crate::engine::Engine::detect() == crate::engine::Engine::TeeSimulatorV4 {
        if let Some(module) = crate::engine::Engine::TeeSimulatorV4.module_dir() {
            if let Ok(content) = std::fs::read_to_string(module.join("module.prop")) {
                if let Some(name) = content.lines().find_map(|line| line.strip_prefix("name=")) {
                    return name.trim().to_string();
                }
            }
        }
        return "TEESimulator v4".to_string();
    }
    if let Ok(entries) = std::fs::read_dir("/data/adb/modules") {
        for entry in entries.flatten() {
            if let Ok(content) = std::fs::read_to_string(entry.path().join("module.prop")) {
                if let Some(name) = content.lines().find_map(|l| l.strip_prefix("name=")) {
                    let name = name.trim();
                    if name.starts_with("TEESimulator") {
                        return name.to_string();
                    }
                }
            }
        }
    }
    for dir in [TS_MODULE, TS_MODULE_HIDDEN] {
        let prop = Path::new(dir).join("module.prop");
        if let Ok(content) = std::fs::read_to_string(&prop) {
            if let Some(name) = content.lines().find_map(|l| l.strip_prefix("name=")) {
                let name = name.trim();
                if !name.is_empty() {
                    return name.to_string();
                }
            }
        }
    }
    "Attestation Engine".to_string()
}

pub fn is_engine_enabled() -> bool {
    crate::engine::Engine::detect().is_enabled()
}

fn detect_nice_name() -> Option<String> {
    if crate::engine::Engine::detect() == crate::engine::Engine::TeeSimulatorV4 {
        // v4's service loop supervises its daemon; avoid competing with it.
        return None;
    }
    for dir in [TS_MODULE, TS_MODULE_HIDDEN] {
        let service_sh = Path::new(dir).join("service.sh");
        if let Ok(content) = std::fs::read_to_string(&service_sh) {
            for line in content.lines() {
                if let Some(pos) = line.find("--nice-name") {
                    let rest = &line[pos..];
                    if let Some(name) = rest.split_whitespace().nth(1) {
                        return Some(name.to_string());
                    }
                }
            }
        }
    }
    None
}

pub fn write_state(state: &HealthState) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(state)?;
    atomic_write(Path::new(HEALTH_STATE), json.as_bytes())
}

pub fn read_state() -> anyhow::Result<HealthState> {
    let content = std::fs::read_to_string(HEALTH_STATE)?;
    let state: HealthState = serde_json::from_str(&content)?;
    Ok(state)
}

pub fn tee_status() -> anyhow::Result<HealthState> {
    let engine = detect_engine();
    let enabled = is_engine_enabled();
    let pid = detect_nice_name().and_then(|n| find_engine_pid(&n));
    let status = if enabled { EngineStatus::Running } else { EngineStatus::Unknown };

    let mut state = read_state().unwrap_or_default();
    state.engine = engine;
    state.pid = pid;
    state.status = status;
    state.last_check = now();
    Ok(state)
}

pub fn check_once(state: &mut HealthState, cfg: &Config) -> anyhow::Result<bool> {
    if crate::engine::Engine::detect().module_dir().is_none() {
        return Ok(true);
    }

    let now_ts = now();
    state.last_check = now_ts;
    state.engine = detect_engine();

    let nice_name = detect_nice_name();

    // Without nice-name (closed-source TrickyStore), just check module enabled status
    if nice_name.is_none() {
        let enabled = is_engine_enabled();
        state.status = if enabled { EngineStatus::Running } else { EngineStatus::Unknown };
        state.pid = None;
        write_state(state)?;
        return Ok(enabled);
    }

    let process_name = nice_name.unwrap();
    let pid = find_engine_pid(&process_name);

    match state.circuit {
        CircuitState::Open => {
            if now_ts.saturating_sub(state.last_restart) >= state.backoff_secs {
                state.circuit = CircuitState::HalfOpen;
                tracing::info!("circuit breaker -> half-open, probing engine");
            } else {
                write_state(state)?;
                return Ok(false);
            }
        }
        CircuitState::HalfOpen => {
            if pid.is_some() {
                state.circuit = CircuitState::Closed;
                state.restarts = 0;
                state.backoff_secs = 0;
                state.status = EngineStatus::Running;
                state.pid = pid;
                tracing::info!("half-open probe succeeded, circuit -> closed");
                write_state(state)?;
                return Ok(true);
            } else {
                state.circuit = CircuitState::Open;
                state.backoff_secs = next_backoff(state.backoff_secs, cfg);
                state.last_restart = now_ts;
                state.status = EngineStatus::Failed;
                tracing::warn!("half-open probe failed, circuit -> open ({}s)", state.backoff_secs);
                write_state(state)?;
                return Ok(false);
            }
        }
        CircuitState::Closed => {}
    }

    if let Some(p) = pid {
        if state.status == EngineStatus::Restarting {
            state.status = EngineStatus::Restarted;
            tracing::info!("engine recovered (pid {})", p);
        } else {
            state.status = EngineStatus::Running;
        }
        state.pid = Some(p);
        write_state(state)?;
        return Ok(true);
    }

    if within_grace_period(state, cfg) {
        state.status = EngineStatus::Unknown;
        write_state(state)?;
        return Ok(true);
    }

    if state.restarts >= cfg.health.max_restarts {
        state.circuit = CircuitState::Open;
        state.backoff_secs = cfg.health.backoff_init as u64;
        state.last_restart = now_ts;
        state.status = EngineStatus::Failed;
        tracing::warn!("max restarts reached, circuit breaker -> open");
        write_state(state)?;
        return Ok(false);
    }

    state.status = EngineStatus::Restarting;
    state.restarts += 1;
    state.last_restart = now_ts;
    tracing::info!("engine dead, restart #{}", state.restarts);

    if let Err(e) = restart_engine(&process_name) {
        tracing::error!("restart failed: {e}");
        state.status = EngineStatus::Failed;
    }

    write_state(state)?;
    Ok(false)
}

pub fn restart_engine(engine_name: &str) -> anyhow::Result<()> {
    let _ = Command::new("killall").arg(engine_name).output();

    let service_sh = [TS_MODULE, TS_MODULE_HIDDEN].iter()
        .map(|d| Path::new(d).join("service.sh"))
        .find(|p| p.exists());

    if let Some(sh) = service_sh {
        let out = Command::new("sh").arg(&sh).output()?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("service.sh failed: {stderr}");
        }
    }
    Ok(())
}

pub fn find_engine_pid(engine: &str) -> Option<u32> {
    Command::new("pidof")
        .arg(engine)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
        })
}

fn within_grace_period(_state: &HealthState, cfg: &Config) -> bool {
    let boot_time = boot_timestamp();
    let grace = cfg.health.grace_period as u64;
    now().saturating_sub(boot_time) < grace
}

fn boot_timestamp() -> u64 {
    std::fs::read_to_string("/proc/stat")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("btime "))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(0)
}

fn next_backoff(current: u64, cfg: &Config) -> u64 {
    let init = cfg.health.backoff_init as u64;
    let cap = cfg.health.backoff_cap as u64;
    if current == 0 {
        init
    } else {
        (current * 2).min(cap)
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
