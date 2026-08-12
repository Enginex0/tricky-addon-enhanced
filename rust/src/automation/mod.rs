pub mod target;
pub mod watcher;

use crate::cli::AutomationAction;
use crate::config::Config;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DaemonStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub target_count: u32,
    pub last_activity: Option<String>,
}

pub fn handle_automation(action: AutomationAction, cfg: &Config) -> anyhow::Result<()> {
    match action {
        AutomationAction::SyncTarget => {
            crate::engine::import_target_mirror()?;
            println!("target synchronized");
            Ok(())
        }
        AutomationAction::ExportTarget => {
            crate::engine::export_target_mirror()?;
            println!("target synchronized");
            Ok(())
        }
        AutomationAction::ProfileReady => {
            crate::engine::ensure_profile_selected()?;
            println!("profile ready");
            Ok(())
        }
        AutomationAction::Profiles => {
            println!(
                "{}",
                serde_json::to_string(&crate::engine::profile_status(
                    &cfg.general.teesim_profile
                )?)?
            );
            Ok(())
        }
        AutomationAction::SelectProfile { name } => {
            crate::engine::validate_profile_choice(name.trim())?;
            let mut current = Config::load(None)?;
            current.set("general.teesim_profile", &name)?;
            Config::backup(None)?;
            current.save(None)?;
            if !name.trim().is_empty() {
                crate::engine::export_target_mirror()?;
                crate::security_patch::handle_security_patch(
                    crate::cli::SecurityPatchAction::ExportLegacy,
                    &current,
                )?;
            }
            println!("TEESimulator profile selection updated");
            Ok(())
        }
        _ if !cfg.automation.enabled => {
            println!("automation disabled");
            Ok(())
        }
        AutomationAction::Status => {
            println!("{}", serde_json::to_string_pretty(&watcher::show_status())?);
            Ok(())
        }
        AutomationAction::Check => {
            let added = watcher::check_new_packages(&cfg.automation.exclude_list, None)?;
            println!("added {added} new packages to target");
            Ok(())
        }
        AutomationAction::Cleanup => {
            let removed = watcher::cleanup_dead_apps()?;
            println!("removed {removed} stale entries from target");
            Ok(())
        }
    }
}
