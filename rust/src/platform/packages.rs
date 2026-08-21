use std::collections::{HashMap, HashSet};
use std::path::Path;

const PACKAGES_LIST: &str = "/data/system/packages.list";
const DATA_APP: &str = "/data/app";

pub fn list_third_party() -> anyhow::Result<HashSet<String>> {
    let output = std::process::Command::new("pm")
        .args(["list", "packages", "-3"])
        .output()?;
    if !output.status.success() {
        anyhow::bail!("pm list packages -3 exited {}", output.status);
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|l| l.strip_prefix("package:"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

pub fn list_all() -> anyhow::Result<HashSet<String>> {
    Ok(list_with_uids()?.into_keys().collect())
}

pub fn list_with_uids() -> anyhow::Result<HashMap<String, u32>> {
    let content = std::fs::read_to_string(PACKAGES_LIST)?;
    Ok(parse_packages_list(&content))
}

fn parse_packages_list(content: &str) -> HashMap<String, u32> {
    content
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            Some((fields.next()?.to_owned(), fields.next()?.parse().ok()?))
        })
        .collect()
}

pub fn resolve_apk_path(package: &str) -> Option<String> {
    if let Some(path) = scan_data_app(package) {
        return Some(path);
    }
    // pm path goes through PMS binder, HMA may filter it
    std::process::Command::new("pm")
        .args(["path", package])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .and_then(|l| l.strip_prefix("package:"))
                .map(|s| s.trim().to_string())
        })
}

fn scan_data_app(package: &str) -> Option<String> {
    let data_app = Path::new(DATA_APP);
    if !data_app.is_dir() {
        return None;
    }

    let prefix = format!("{package}-");

    for entry in std::fs::read_dir(data_app).ok()?.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();

        // Android 12+: ~~<random>/<pkg>-<random>/base.apk
        if name.starts_with("~~") {
            if let Some(path) = scan_subdir(&entry.path(), &prefix) {
                return Some(path);
            }
            continue;
        }

        // Older: <pkg>-<n>/base.apk
        if name.starts_with(prefix.as_str()) {
            let apk = entry.path().join("base.apk");
            if apk.exists() {
                return Some(apk.to_string_lossy().into_owned());
            }
        }
    }

    None
}

fn scan_subdir(parent: &Path, prefix: &str) -> Option<String> {
    for entry in std::fs::read_dir(parent).ok()?.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(prefix) {
            let apk = entry.path().join("base.apk");
            if apk.exists() {
                return Some(apk.to_string_lossy().into_owned());
            }
        }
    }
    None
}
