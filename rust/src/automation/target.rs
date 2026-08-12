use crate::platform::fs::atomic_write;
use std::path::Path;

pub(crate) const AUTO_ADDED: &str = "/data/adb/tricky_store/.automation/auto_added.txt";
const TARGET_FILE: &str = "/data/adb/tricky_store/target.txt";

pub fn read_target() -> anyhow::Result<Vec<String>> {
    Ok(crate::engine::read_targets()?
        .into_iter()
        .map(|line| strip_suffix(&line).to_owned())
        .collect())
}

pub fn read_target_raw() -> anyhow::Result<Vec<String>> {
    if crate::engine::Engine::detect() == crate::engine::Engine::TrickyStore {
        let path = Path::new(TARGET_FILE);
        if !path.exists() {
            return Ok(Vec::new());
        }
        return Ok(std::fs::read_to_string(path)?
            .lines()
            .map(|line| line.trim().to_owned())
            .filter(|line| !line.is_empty())
            .collect());
    }
    crate::engine::read_targets()
}

pub fn write_target(entries: &[String]) -> anyhow::Result<()> {
    if crate::engine::Engine::detect() == crate::engine::Engine::TrickyStore {
        let mut content = entries.join("\n");
        if !content.is_empty() {
            content.push('\n');
        }
        return atomic_write(Path::new(TARGET_FILE), content.as_bytes());
    }
    crate::engine::write_targets(entries)
}

pub fn add_package(pkg: &str, exclude_list: &[String]) -> anyhow::Result<bool> {
    if is_excluded(pkg, exclude_list) {
        return Ok(false);
    }

    let lines = read_target_raw()?;
    let bare = strip_suffix(pkg);

    for line in &lines {
        if strip_suffix(line) == bare {
            return Ok(false);
        }
    }

    let mut lines = lines;
    lines.push(pkg.to_string());
    write_target(&lines)?;
    Ok(true)
}

pub fn remove_package(pkg: &str) -> anyhow::Result<bool> {
    let lines = read_target_raw()?;
    let original_len = lines.len();
    let bare = strip_suffix(pkg);
    let filtered: Vec<String> = lines
        .into_iter()
        .filter(|l| strip_suffix(l) != bare)
        .collect();

    let changed = filtered.len() != original_len;
    if changed {
        write_target(&filtered)?;
    }
    Ok(changed)
}

pub(crate) fn record_auto_added(pkg: &str) -> anyhow::Result<()> {
    let path = Path::new(AUTO_ADDED);
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let bare = strip_suffix(pkg);
    if existing.lines().any(|l| strip_suffix(l.trim()) == bare) {
        return Ok(());
    }
    let mut content = existing;
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(pkg);
    content.push('\n');
    atomic_write(path, content.as_bytes())
}

pub(crate) fn forget_auto_added(pkg: &str) -> anyhow::Result<()> {
    let path = Path::new(AUTO_ADDED);
    if !path.exists() {
        return Ok(());
    }
    let bare = strip_suffix(pkg);
    let mut content = String::new();
    for line in std::fs::read_to_string(path)?.lines() {
        if strip_suffix(line.trim()) != bare {
            content.push_str(line);
            content.push('\n');
        }
    }
    atomic_write(path, content.as_bytes())
}

pub(crate) fn write_auto_added(entries: &[String]) -> anyhow::Result<()> {
    let mut content = entries.join("\n");
    if !content.is_empty() {
        content.push('\n');
    }
    atomic_write(Path::new(AUTO_ADDED), content.as_bytes())
}

fn is_excluded(pkg: &str, exclude_list: &[String]) -> bool {
    exclude_list.iter().any(|pattern| {
        if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            pkg.starts_with(prefix)
        } else {
            pkg == pattern
        }
    })
}

fn strip_suffix(s: &str) -> &str {
    s.trim_end_matches('!').trim_end_matches('?')
}
