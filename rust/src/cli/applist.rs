use super::ApplistAction;
use crate::automation::target;
use crate::automation::watcher;
use crate::config::Config;
use crate::platform::packages;
use serde::Serialize;
use std::path::Path;
use std::process::Command;

const TA_DIR: &str = "/data/adb/tricky_store/ta-enhanced";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppEntry {
    package: String,
    name: Option<String>,
    is_xposed: bool,
    suffix: Option<String>,
}

pub fn handle_applist(action: ApplistAction, cfg: &Config) -> anyhow::Result<()> {
    match action {
        ApplistAction::List => {
            let entries = build_applist(cfg)?;
            serde_json::to_writer(std::io::stdout(), &entries)?;
            println!();
        }
        ApplistAction::Name { package } => {
            let name = resolve_app_name(&package)?;
            println!("{name}");
        }
        ApplistAction::Save => {
            let input = std::io::read_to_string(std::io::stdin())?;
            let pending = Path::new(TA_DIR).join("applist.pending");
            crate::platform::fs::atomic_write(&pending, input.as_bytes())?;
        }
        ApplistAction::Xposed => {
            let modules = detect_xposed_modules()?;
            serde_json::to_writer(std::io::stdout(), &modules)?;
            println!();
        }
    }
    Ok(())
}

fn build_applist(cfg: &Config) -> anyhow::Result<Vec<AppEntry>> {
    let raw = target::read_target_raw()?;
    let exclude = &cfg.automation.exclude_list;

    let mut entries = Vec::with_capacity(raw.len());
    for line in &raw {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        let suffix = if line.ends_with('!') {
            Some("!".to_string())
        } else if line.ends_with('?') {
            Some("?".to_string())
        } else {
            None
        };

        let bare = line.trim_end_matches('!').trim_end_matches('?');
        let is_excluded = exclude.iter().any(|p| {
            if p.ends_with('*') {
                bare.starts_with(&p[..p.len() - 1])
            } else {
                bare == p
            }
        });
        if is_excluded {
            continue;
        }

        entries.push(AppEntry {
            package: bare.to_string(),
            name: resolve_app_name(bare).ok(),
            is_xposed: watcher::is_xposed_module(bare),
            suffix,
        });
    }

    Ok(entries)
}

fn resolve_app_name(package: &str) -> anyhow::Result<String> {
    let output = Command::new("dumpsys")
        .args(["package", package])
        .output()?;

    if !output.status.success() {
        anyhow::bail!("dumpsys package {package} failed");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("applicationInfo=") {
            if let Some(label_start) = rest.find("label=") {
                let after = &rest[label_start + 6..];
                let label = after.split_whitespace().next().unwrap_or(package);
                return Ok(label.to_string());
            }
        }
    }

    Ok(package.to_string())
}

fn detect_xposed_modules() -> anyhow::Result<Vec<String>> {
    let all = packages::list_third_party()?;
    let mut modules: Vec<String> = all
        .into_iter()
        .filter(|pkg| watcher::is_xposed_module(pkg))
        .collect();
    modules.sort();
    Ok(modules)
}
