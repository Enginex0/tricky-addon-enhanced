use std::collections::HashMap;
use std::path::Path;

use crate::config::Config;

const MIGRATION_MAP: &[(&str, &str)] = &[
    ("keybox_source", "keybox.source"),
    ("keybox_enabled", "keybox.enabled"),
    ("keybox_interval", "keybox.interval"),
    ("security_patch_auto", "security_patch.auto_update"),
    ("security_patch_interval", "security_patch.interval"),
    ("vbhash_enabled", "vbhash.enabled"),
    ("conflict_check_enabled", "conflict.enabled"),
    ("automation_target_enabled", "automation.enabled"),
    ("ui_language", "ui.language"),
];

pub fn migrate_ini_to_toml(ini_path: &Path, toml_path: &Path) -> anyhow::Result<()> {
    if toml_path.exists() {
        tracing::info!(
            "TOML config already exists at {}, skipping migration",
            toml_path.display()
        );
        return Ok(());
    }

    if ini_path.exists() {
        let bak = ini_path.with_extension("conf.bak");
        std::fs::copy(ini_path, &bak)?;
        tracing::info!("backed up {} -> {}", ini_path.display(), bak.display());
    }

    let mut config = Config::default();

    if ini_path.exists() {
        let content = std::fs::read_to_string(ini_path)?;
        let ini_map = parse_ini(&content);

        for (ini_key, toml_path_key) in MIGRATION_MAP {
            if let Some(value) = ini_map.get(*ini_key) {
                config.set(toml_path_key, value).ok();
            }
        }

        for key in ini_map.keys() {
            if !MIGRATION_MAP.iter().any(|(k, _)| k == key) {
                tracing::warn!(
                    "unmapped legacy config key: {} (preserved in .conf.bak)",
                    key
                );
            }
        }
    }

    let ts_dir = Path::new("/data/adb/tricky_store");
    if ts_dir.join("security_patch_auto_config").exists() {
        config.security_patch.auto_update = true;
    }
    if ts_dir.join("target_from_denylist").exists() {
        config.automation.merge_denylist = true;
    }

    config.save(Some(toml_path))?;

    let sentinel = toml_path.with_file_name(".migrated");
    std::fs::write(&sentinel, "")?;
    tracing::info!("migrated {} -> {}", ini_path.display(), toml_path.display());

    Ok(())
}

fn parse_ini(content: &str) -> HashMap<String, String> {
    content
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .filter_map(|l| {
            let mut parts = l.splitn(2, '=');
            let key = parts.next()?.trim().to_string();
            let value = parts.next()?.trim().to_string();
            Some((key, value))
        })
        .collect()
}
