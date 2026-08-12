use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use tracing::warn;

use crate::config::Config;
use crate::platform::packages;

const TS_MODULE_PROP: &str = "/data/adb/modules/tricky_store/module.prop";
const TS_MODULE_PROP_HIDDEN: &str = "/data/adb/modules/.tricky_store/module.prop";
const ORIGINAL_DESC_FILE: &str = "/data/adb/tricky_store/ta-enhanced/description.bak";
const BOOT_HASH_FILE: &str = "/data/adb/boot_hash";
const SECURITY_PATCH_FILE: &str = "/data/adb/tricky_store/security_patch.txt";

struct DescStrings {
    apps: &'static str,
    app_singular: &'static str,
}

fn desc_strings(lang: &str) -> DescStrings {
    match lang {
        "ar" => DescStrings { apps: "\u{062a}\u{0637}\u{0628}\u{064a}\u{0642}\u{0627}\u{062a}", app_singular: "\u{062a}\u{0637}\u{0628}\u{064a}\u{0642}" },
        "az" => DescStrings { apps: "T\u{0259}tbiq", app_singular: "T\u{0259}tbiq" },
        "bn" => DescStrings { apps: "\u{0985}\u{09cd}\u{09af}\u{09be}\u{09aa}\u{09cd}\u{09b8}", app_singular: "\u{0985}\u{09cd}\u{09af}\u{09be}\u{09aa}" },
        "de" => DescStrings { apps: "Apps", app_singular: "App" },
        "el" => DescStrings { apps: "\u{0395}\u{03c6}\u{03b1}\u{03c1}\u{03bc}\u{03bf}\u{03b3}\u{03ad}\u{03c2}", app_singular: "\u{0395}\u{03c6}\u{03b1}\u{03c1}\u{03bc}\u{03bf}\u{03b3}\u{03ae}" },
        "es-ES" => DescStrings { apps: "Apps", app_singular: "App" },
        "fa" => DescStrings { apps: "\u{0628}\u{0631}\u{0646}\u{0627}\u{0645}\u{0647}\u{200c}\u{0647}\u{0627}", app_singular: "\u{0628}\u{0631}\u{0646}\u{0627}\u{0645}\u{0647}" },
        "fr" => DescStrings { apps: "Apps", app_singular: "App" },
        "id" => DescStrings { apps: "Aplikasi", app_singular: "Aplikasi" },
        "it" => DescStrings { apps: "App", app_singular: "App" },
        "ja" => DescStrings { apps: "\u{30a2}\u{30d7}\u{30ea}", app_singular: "\u{30a2}\u{30d7}\u{30ea}" },
        "ko" => DescStrings { apps: "\u{c571}", app_singular: "\u{c571}" },
        "pl" => DescStrings { apps: "Aplikacji", app_singular: "Aplikacja" },
        "pt-BR" => DescStrings { apps: "Apps", app_singular: "App" },
        "ru" => DescStrings { apps: "\u{043f}\u{0440}\u{0438}\u{043b}\u{043e}\u{0436}\u{0435}\u{043d}\u{0438}\u{0439}", app_singular: "\u{043f}\u{0440}\u{0438}\u{043b}\u{043e}\u{0436}\u{0435}\u{043d}\u{0438}\u{0435}" },
        "th" => DescStrings { apps: "\u{0e41}\u{0e2d}\u{0e1b}", app_singular: "\u{0e41}\u{0e2d}\u{0e1b}" },
        "tl" => DescStrings { apps: "Apps", app_singular: "App" },
        "tr" => DescStrings { apps: "Uygulama", app_singular: "Uygulama" },
        "uk" => DescStrings { apps: "\u{0434}\u{043e}\u{0434}\u{0430}\u{0442}\u{043a}\u{0456}\u{0432}", app_singular: "\u{0434}\u{043e}\u{0434}\u{0430}\u{0442}\u{043e}\u{043a}" },
        "vi" => DescStrings { apps: "\u{1ee9}ng d\u{1ee5}ng", app_singular: "\u{1ee9}ng d\u{1ee5}ng" },
        "zh-CN" => DescStrings { apps: "\u{4e2a}\u{5e94}\u{7528}", app_singular: "\u{4e2a}\u{5e94}\u{7528}" },
        "zh-TW" => DescStrings { apps: "\u{500b}\u{61c9}\u{7528}", app_singular: "\u{500b}\u{61c9}\u{7528}" },
        _ => DescStrings { apps: "Apps", app_singular: "App" },
    }
}

pub fn handle_status(action: crate::cli::StatusAction, cfg: &Config) -> Result<()> {
    use crate::cli::StatusAction;
    match action {
        StatusAction::Update => {
            save_original_description()?;
            let desc = build_description(cfg);
            update_prop_description(&desc)?;
            println!("{desc}");
            Ok(())
        }
        StatusAction::XposedScan => {
            let modules = scan_xposed()?;
            for m in &modules {
                println!("{m}");
            }
            Ok(())
        }
    }
}

pub fn build_description(cfg: &Config) -> String {
    let count = count_active_apps();
    let keybox = get_keybox_label(cfg);
    let patch = get_patch_level();
    let vbhash_active = get_vbhash_active();
    let strings = desc_strings(&cfg.ui.language);

    let app_label = if count == 1 { strings.app_singular } else { strings.apps };

    if cfg.status.emoji {
        let vb = if vbhash_active { "\u{1f512} VBHash" } else { "\u{1f513} VBHash" };
        format!("\u{26a1} {count} {app_label} | \u{1f511} {keybox} | \u{1f6e1}\u{fe0f} {patch} | {vb}")
    } else {
        let vb = if vbhash_active { "active" } else { "inactive" };
        format!("{count} {app_label} | {keybox} | {patch} | VBHash: {vb}")
    }
}

pub fn count_active_apps() -> u32 {
    let targets = match crate::engine::read_targets() {
        Ok(targets) => targets,
        Err(_) => return 0,
    };

    let target_pkgs: HashSet<&str> = targets
        .iter()
        .map(String::as_str)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('!'))
        .collect();

    if target_pkgs.is_empty() {
        return 0;
    }

    let installed = match packages::list_all() {
        Ok(set) => set,
        Err(_) => return 0,
    };

    target_pkgs.iter().filter(|pkg| installed.contains(**pkg)).count() as u32
}

pub fn get_keybox_label(cfg: &Config) -> &'static str {
    match cfg.keybox.source.as_str() {
        "yurikey" => "Yurikey",
        "upstream" => "Upstream",
        "custom" => "Custom",
        _ => "Unknown",
    }
}

pub fn get_patch_level() -> String {
    if let Ok((system, boot, _)) = crate::engine::read_patch_dates() {
        let value = if boot.is_empty() { system } else { boot };
        if !value.is_empty() {
            return value;
        }
    }
    if let Ok(content) = std::fs::read_to_string(SECURITY_PATCH_FILE) {
        for line in content.lines() {
            if let Some(val) = line.strip_prefix("boot=") {
                let trimmed = val.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }
    crate::platform::props::getprop("ro.build.version.security_patch")
        .unwrap_or_else(|| "unknown".into())
}

pub fn get_vbhash_active() -> bool {
    std::fs::read_to_string(BOOT_HASH_FILE)
        .map(|h| {
            let t = h.trim();
            t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit())
        })
        .unwrap_or(false)
}

pub fn save_original_description() -> Result<()> {
    let desc_path = original_description_path();
    if desc_path.exists() {
        return Ok(());
    }
    let current = read_module_prop_desc();
    if let Some(desc) = current {
        if let Some(parent) = desc_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&desc_path, desc.as_bytes())
            .context("failed to save original description")?;
    }
    Ok(())
}

pub fn restore_original_description() -> Result<()> {
    let desc_path = original_description_path();
    if !desc_path.exists() {
        return Ok(());
    }
    let original = std::fs::read_to_string(&desc_path)
        .context("failed to read original description")?;
    update_prop_description(&original)?;
    Ok(())
}

fn original_description_path() -> std::path::PathBuf {
    if crate::engine::Engine::detect() == crate::engine::Engine::TeeSimulatorV4 {
        std::path::PathBuf::from("/data/adb/tricky_store/ta-enhanced/addon-description.bak")
    } else {
        std::path::PathBuf::from(ORIGINAL_DESC_FILE)
    }
}

pub fn update_prop_description(desc: &str) -> Result<()> {
    let prop_path = find_module_prop()
        .ok_or_else(|| anyhow::anyhow!("module.prop not found"))?;

    let content = std::fs::read_to_string(&prop_path)
        .context("failed to read module.prop")?;

    let mut found = false;
    let new_content: String = content
        .lines()
        .map(|line| {
            if line.starts_with("description=") {
                found = true;
                format!("description={desc}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    if !found {
        warn!("no description= line in module.prop");
        return Ok(());
    }

    let trailing = if content.ends_with('\n') { "\n" } else { "" };
    crate::platform::fs::atomic_write(
        Path::new(&prop_path),
        format!("{new_content}{trailing}").as_bytes(),
    )
    .context("failed to write module.prop")?;

    push_live_description(desc);
    Ok(())
}

fn push_live_description(desc: &str) {
    let ksud = ["/data/adb/ksu/bin/ksud", "/data/adb/ap/bin/ksud"]
        .iter()
        .find(|p| Path::new(p).exists())
        .copied()
        .unwrap_or("ksud");

    // ksud requires --internal <module-id>; the KSU_MODULE env var is ignored.
    let module_id = if crate::engine::Engine::detect() == crate::engine::Engine::TeeSimulatorV4 {
        "TA_enhanced"
    } else {
        "tricky_store"
    };
    let _ = Command::new(ksud)
        .args([
            "module",
            "config",
            "--internal",
            module_id,
            "set",
            "override.description",
            desc,
        ])
        .output();
}

pub fn scan_xposed() -> Result<Vec<String>> {
    let all = packages::list_third_party()
        .context("failed to read packages.list")?;

    let xposed_indicators = [
        "xposed", "lsposed", "edxposed", "riru",
        "shamiko", "zygisk",
    ];

    let mut found: Vec<String> = all
        .into_iter()
        .filter(|pkg| {
            let lower = pkg.to_ascii_lowercase();
            xposed_indicators.iter().any(|ind| lower.contains(ind))
        })
        .collect();
    found.sort();
    Ok(found)
}

fn find_module_prop() -> Option<String> {
    let candidates: &[&str] = if crate::engine::Engine::detect() == crate::engine::Engine::TeeSimulatorV4 {
        &[
            "/data/adb/modules/.TA_enhanced/module.prop",
            "/data/adb/modules/TA_enhanced/module.prop",
        ]
    } else {
        &[TS_MODULE_PROP, TS_MODULE_PROP_HIDDEN]
    };
    candidates
        .iter()
        .find(|p| Path::new(p).exists())
        .map(|p| p.to_string())
}

fn read_module_prop_desc() -> Option<String> {
    let prop_path = find_module_prop()?;
    let content = std::fs::read_to_string(prop_path).ok()?;
    content
        .lines()
        .find(|l| l.starts_with("description="))
        .map(|l| l.trim_start_matches("description=").to_string())
}
