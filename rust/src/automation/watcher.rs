use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use crate::platform::fs::{atomic_write, ensure_dir};
use crate::platform::packages;
use super::target;
use super::DaemonStatus;

const AUTOMATION_DIR: &str = "/data/adb/tricky_store/.automation";
const KNOWN_PACKAGES: &str = "/data/adb/tricky_store/.automation/known_packages.txt";
const EXCLUDE_PATTERNS: &str = "/data/adb/tricky_store/.automation/exclude_patterns.txt";

pub fn check_new_packages(exclude_list: &[String], manager: Option<&str>) -> anyhow::Result<u32> {
    ensure_dir(Path::new(AUTOMATION_DIR))?;

    let current = packages::list_third_party()?;
    let known = load_known_packages();
    let effective_excludes = merge_excludes(exclude_list);

    let new_pkgs: Vec<String> = current
        .iter()
        .filter(|p| !known.contains(*p))
        .cloned()
        .collect();

    let mut added = 0u32;
    for pkg in &new_pkgs {
        if is_xposed_module(pkg) {
            continue;
        }
        if target::add_package(pkg, &effective_excludes)? {
            let _ = target::record_auto_added(pkg);
            added += 1;
            tracing::info!("added {pkg} to target");
        }
    }

    // One-shot retroactive attribution. When auto_added.txt is missing, the
    // legacy in-memory `known` set still reflects what the previous (broken)
    // daemon scan saw — the intersection with target.txt and system apps is
    // the best available estimate of daemon-injected entries from v5.39/v5.40.
    // Must run BEFORE save_known_packages overwrites the legacy snapshot.
    if !Path::new(target::AUTO_ADDED).exists() {
        match bootstrap_auto_added(&known) {
            Ok(count) => tracing::info!("auto_added bootstrap: attributed {count} legacy entries"),
            Err(e) => {
                tracing::warn!("auto_added bootstrap failed, retrying next tick: {e}");
                return Ok(added);
            }
        }
    }

    save_known_packages(&current)?;

    if added > 0 {
        if let Some(mgr) = manager {
            refresh_root_manager(mgr);
        }
    }

    Ok(added)
}

pub fn cleanup_dead_apps() -> anyhow::Result<u32> {
    // list_all (every package in packages.list) so we don't strip system apps
    // that were intentionally seeded by install scripts.
    let installed = packages::list_all()?;
    let target_list = target::read_target()?;
    let mut removed = 0u32;

    for pkg in &target_list {
        if pkg.starts_with("uid:") || installed.contains(pkg) || app_data_exists(pkg) {
            continue;
        }
        if target::remove_package(pkg)? {
            let _ = target::forget_auto_added(pkg);
            removed += 1;
            tracing::info!("removed uninstalled {pkg} from target");
        }
    }

    Ok(removed)
}

pub fn refresh_root_manager(manager: &str) {
    let pkg = match manager {
        "KSU" => "me.weishu.kernelsu",
        "APATCH" => "me.bmax.apatch",
        "MAGISK" => "com.topjohnwu.magisk",
        _ => return,
    };
    if Command::new("pm").args(["path", pkg]).output()
        .map(|o| o.status.success()).unwrap_or(false)
    {
        let _ = Command::new("am").args(["force-stop", pkg]).output();
    }
}

pub fn is_xposed_module(package: &str) -> bool {
    let apk_path = match packages::resolve_apk_path(package) {
        Some(p) => p,
        None => return false,
    };

    if let Ok(output) = Command::new("unzip")
        .args(["-l", &apk_path])
        .output()
    {
        let listing = String::from_utf8_lossy(&output.stdout);
        if listing.contains("assets/xposed_init")
            || listing.contains("META-INF/xposed/module.prop")
        {
            return true;
        }
    }

    if let Ok(output) = Command::new("unzip")
        .args(["-p", &apk_path, "AndroidManifest.xml"])
        .output()
    {
        let manifest: Vec<u8> = output.stdout.into_iter().filter(|b| *b != 0).collect();
        if String::from_utf8_lossy(&manifest).contains("xposedmodule") {
            return true;
        }
    }
    false
}

pub fn show_status() -> DaemonStatus {
    let pid_path = Path::new("/data/adb/tricky_store/ta-enhanced/daemon.pid");
    let pid = crate::platform::process::read_pid(pid_path);
    let running = pid.map(crate::platform::process::is_running).unwrap_or(false);
    let target_count = target::read_target().map(|t| t.len() as u32).unwrap_or(0);
    let last_activity = std::fs::metadata(KNOWN_PACKAGES)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs().to_string());

    DaemonStatus {
        running,
        pid: pid.map(|p| p as u32),
        target_count,
        last_activity,
    }
}

fn app_data_exists(pkg: &str) -> bool {
    Path::new(&format!("/data/data/{pkg}")).exists()
}

fn bootstrap_auto_added(legacy_known: &HashSet<String>) -> anyhow::Result<usize> {
    let target_set: HashSet<String> = target::read_target()?.into_iter().collect();
    let system_apps = list_system_apps()?;

    let entries: Vec<String> = legacy_known
        .iter()
        .filter(|p| target_set.contains(*p))
        .filter(|p| system_apps.contains(*p))
        .cloned()
        .collect();

    let count = entries.len();
    target::write_auto_added(&entries)?;
    Ok(count)
}

fn list_system_apps() -> anyhow::Result<HashSet<String>> {
    let output = Command::new("pm")
        .args(["list", "packages", "-s"])
        .output()?;
    if !output.status.success() {
        anyhow::bail!("pm list packages -s exited {}", output.status);
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|l| l.strip_prefix("package:"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

fn merge_excludes(config_list: &[String]) -> Vec<String> {
    let file_entries = std::fs::read_to_string(EXCLUDE_PATTERNS)
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect::<Vec<_>>();
    let mut merged: HashSet<String> = file_entries.into_iter().collect();
    merged.extend(config_list.iter().cloned());
    merged.into_iter().collect()
}

fn load_known_packages() -> HashSet<String> {
    std::fs::read_to_string(KNOWN_PACKAGES)
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

fn save_known_packages(pkgs: &HashSet<String>) -> anyhow::Result<()> {
    let mut sorted: Vec<&String> = pkgs.iter().collect();
    sorted.sort();
    let content = sorted.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n");
    let mut data = content;
    if !data.is_empty() {
        data.push('\n');
    }
    atomic_write(Path::new(KNOWN_PACKAGES), data.as_bytes())
}
