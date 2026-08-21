use super::*;
use crate::config::Config;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn read_version_from_prop() -> String {
    [
        "/data/adb/modules/.TA_enhanced/module.prop",
        "/data/adb/modules/TA_enhanced/module.prop",
    ]
    .iter()
    .find_map(|p| std::fs::read_to_string(p).ok())
    .and_then(|s| {
        s.lines()
            .find(|l| l.starts_with("version="))
            .map(|l| l.trim_start_matches("version=").to_string())
    })
    .unwrap_or_else(|| VERSION.to_string())
}

pub fn dispatch(command: Commands, cfg: &Config) -> anyhow::Result<()> {
    match command {
        Commands::Version => {
            println!("{}", read_version_from_prop());
            Ok(())
        }
        Commands::Daemon { manager } => crate::daemon::handle_daemon(cfg, manager.as_deref()),
        Commands::DaemonStop => crate::daemon::handle_daemon_stop(),
        Commands::Config { action } => crate::config::handle_config(action, cfg),
        Commands::Keybox { action } => crate::keybox::handle_keybox(action, cfg),
        Commands::SecurityPatch { action } => {
            crate::security_patch::handle_security_patch(action, cfg)
        }
        Commands::Conflict { action } => crate::conflict::handle_conflict(action, cfg),
        Commands::Vbhash { action } => crate::vbhash::handle_vbhash(action, cfg),
        Commands::Health { action } => crate::health::handle_health(action, cfg),
        Commands::Status { action } => crate::status::handle_status(action, cfg),
        Commands::Automation { action } => crate::automation::handle_automation(action, cfg),
        Commands::WebuiInit => crate::cli::webui_init::handle_webui_init(cfg),
        Commands::Applist { action } => crate::cli::applist::handle_applist(action, cfg),
        Commands::Module { action } => crate::module::handle_module(action, cfg),
        Commands::DaemonStatus => crate::daemon::handle_daemon_status(),
    }
}
