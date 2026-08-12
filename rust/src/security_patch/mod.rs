pub mod bulletin;

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use tracing::{info, warn};

use crate::cli::SecurityPatchAction;
use crate::config::Config;
use crate::platform::fs::atomic_write;
use crate::platform::props::{getprop, set as prop_set};

const TS_DIR: &str = "/data/adb/tricky_store";
const SECURITY_PATCH_FILE: &str = "/data/adb/tricky_store/security_patch.txt";
const DEVCONFIG_TOML: &str = "/data/adb/tricky_store/devconfig.toml";
const TS_MODULE_PROP: &str = "/data/adb/modules/tricky_store/module.prop";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TrickyStoreVariant {
    TeeSimulatorV4,
    James,
    Standard,
    Legacy,
}

impl std::fmt::Display for TrickyStoreVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TeeSimulatorV4 => write!(f, "TEESimulator v4"),
            Self::James => write!(f, "James"),
            Self::Standard => write!(f, "Standard"),
            Self::Legacy => write!(f, "Legacy"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PatchDates {
    pub system: String,
    pub boot: String,
    pub vendor: String,
}

pub fn handle_security_patch(action: SecurityPatchAction, cfg: &Config) -> Result<()> {
    match action {
        SecurityPatchAction::Set => {
            set(cfg)?;
            println!("security patch dates applied");
            Ok(())
        }
        SecurityPatchAction::Update { force } => {
            if force {
                update_force()?;
            } else {
                update(cfg)?;
            }
            println!("security patch dates updated");
            Ok(())
        }
        SecurityPatchAction::Show => {
            let output = show_current()?;
            println!("{output}");
            Ok(())
        }
        SecurityPatchAction::SetCustom {
            system,
            boot,
            vendor,
        } => {
            set_custom(&system, &boot, &vendor)?;
            println!("custom security patch dates applied");
            Ok(())
        }
        SecurityPatchAction::ImportLegacy => {
            if crate::engine::Engine::detect() != crate::engine::Engine::TeeSimulatorV4 {
                bail!("legacy patch import is only available for TEESimulator v4");
            }
            crate::engine::import_legacy_patch()?;
            println!("legacy security patch imported");
            Ok(())
        }
        SecurityPatchAction::ExportLegacy => {
            if crate::engine::Engine::detect() != crate::engine::Engine::TeeSimulatorV4 {
                return Ok(());
            }
            let (system, boot, vendor) = crate::engine::read_patch_dates()?;
            let content = format!("system={system}\nboot={boot}\nvendor={vendor}\n");
            atomic_write(Path::new(SECURITY_PATCH_FILE), content.as_bytes())?;
            println!("TEESimulator security patch exported");
            Ok(())
        }
    }
}

pub fn read_device_dates() -> PatchDates {
    PatchDates {
        system: get_system_patch_date().unwrap_or_default(),
        boot: get_boot_patch_date().unwrap_or_default(),
        vendor: get_vendor_patch_date().unwrap_or_default(),
    }
}

pub fn get_system_patch_date() -> Option<String> {
    getprop("ro.build.version.security_patch")
}

pub fn get_boot_patch_date() -> Option<String> {
    getprop("ro.bootimage.build.date.security_patch")
        .or_else(|| getprop("ro.vendor.build.security_patch"))
        .or_else(|| getprop("ro.build.version.security_patch"))
}

pub fn get_vendor_patch_date() -> Option<String> {
    getprop("ro.vendor.build.security_patch").or_else(|| getprop("ro.build.version.security_patch"))
}

pub fn detect_variant() -> TrickyStoreVariant {
    if crate::engine::Engine::detect() == crate::engine::Engine::TeeSimulatorV4 {
        return TrickyStoreVariant::TeeSimulatorV4;
    }
    let prop_content = std::fs::read_to_string(TS_MODULE_PROP).unwrap_or_default();

    if prop_content.contains("James") && !prop_content.contains("beakthoven") {
        return TrickyStoreVariant::James;
    }

    if prop_content.contains("TEESimulator") || prop_content.contains("beakthoven") {
        return TrickyStoreVariant::Standard;
    }

    if let Some(version) = extract_version_code(&prop_content) {
        if version >= 158 {
            return TrickyStoreVariant::Standard;
        }
    }

    TrickyStoreVariant::Legacy
}

pub fn set(config: &Config) -> Result<()> {
    let dates = if config.security_patch.custom_date.is_empty() {
        read_device_dates()
    } else {
        PatchDates {
            system: config.security_patch.custom_date.clone(),
            boot: config.security_patch.custom_date.clone(),
            vendor: config.security_patch.custom_date.clone(),
        }
    };

    if dates.system.is_empty() {
        bail!("cannot read system security patch date");
    }

    let variant = detect_variant();
    info!("detected TrickyStore variant: {variant}");
    if patch_file_already_matches(&variant, &dates) {
        return Ok(());
    }
    write_patch_dates(&variant, &dates)
}

fn patch_file_already_matches(variant: &TrickyStoreVariant, dates: &PatchDates) -> bool {
    match variant {
        TrickyStoreVariant::TeeSimulatorV4 => crate::engine::read_patch_dates()
            .map(|current| {
                current
                    == (
                        dates.system.clone(),
                        dates.boot.clone(),
                        dates.vendor.clone(),
                    )
            })
            .unwrap_or(false),
        TrickyStoreVariant::Standard => {
            let Ok(content) = std::fs::read_to_string(SECURITY_PATCH_FILE) else {
                return false;
            };
            content.contains(&format!("system={}", dates.system))
                && content.contains(&format!("boot={}", dates.boot))
                && content.contains(&format!("vendor={}", dates.vendor))
        }
        TrickyStoreVariant::James => {
            let Ok(content) = std::fs::read_to_string(DEVCONFIG_TOML) else {
                return false;
            };
            content.contains(&format!("securityPatch = \"{}\"", dates.system))
        }
        TrickyStoreVariant::Legacy => false,
    }
}

pub fn set_custom(system: &str, boot: &str, vendor: &str) -> Result<()> {
    let device = read_device_dates();
    let dates = PatchDates {
        system: if system == "prop" {
            device.system
        } else {
            system.to_string()
        },
        boot: if boot == "prop" {
            device.boot
        } else {
            boot.to_string()
        },
        vendor: if vendor == "prop" {
            device.vendor
        } else {
            vendor.to_string()
        },
    };

    let variant = detect_variant();
    write_patch_dates(&variant, &dates)
}

pub fn update(config: &Config) -> Result<()> {
    if !config.security_patch.auto_update {
        info!("security patch auto-update is disabled");
        return Ok(());
    }
    if !config.security_patch.custom_date.is_empty() {
        info!(
            "enforcing user custom patch: {}",
            config.security_patch.custom_date
        );
        return set(config);
    }
    update_force()
}

pub fn update_force() -> Result<()> {
    let latest = bulletin::fetch_latest_patch()?;
    let dates = PatchDates {
        system: latest.clone(),
        boot: latest.clone(),
        vendor: latest,
    };

    let variant = detect_variant();
    info!(
        "updating security patch to {} (variant: {variant})",
        dates.system
    );
    write_patch_dates(&variant, &dates)
}

pub fn show_current() -> Result<String> {
    let variant = detect_variant();
    let dates = read_device_dates();
    let mut output = String::new();

    output.push_str(&format!("variant: {variant}\n"));
    output.push_str(&format!("system: {}\n", dates.system));
    output.push_str(&format!("boot: {}\n", dates.boot));
    output.push_str(&format!("vendor: {}\n", dates.vendor));

    match variant {
        TrickyStoreVariant::TeeSimulatorV4 => {
            if let Ok((system, boot, vendor)) = crate::engine::read_patch_dates() {
                output.push_str(&format!(
                    "config_content:\nsystem={system}\nboot={boot}\nvendor={vendor}\n"
                ));
            }
            output.push_str("config_file: /data/adb/teesim/config.json\n");
        }
        TrickyStoreVariant::James => {
            let path = Path::new(DEVCONFIG_TOML);
            if path.exists() {
                let content = std::fs::read_to_string(path)?;
                if let Some(line) = content.lines().find(|l| l.starts_with("securityPatch")) {
                    output.push_str(&format!("config: {line}\n"));
                }
            }
            output.push_str(&format!("config_file: {DEVCONFIG_TOML}\n"));
        }
        TrickyStoreVariant::Standard => {
            let path = Path::new(SECURITY_PATCH_FILE);
            if path.exists() {
                let content = std::fs::read_to_string(path)?;
                output.push_str(&format!("config_content:\n{content}"));
            }
            output.push_str(&format!("config_file: {SECURITY_PATCH_FILE}\n"));
        }
        TrickyStoreVariant::Legacy => {
            output.push_str("config_file: (resetprop only)\n");
        }
    }

    Ok(output)
}

fn write_patch_dates(variant: &TrickyStoreVariant, dates: &PatchDates) -> Result<()> {
    match variant {
        TrickyStoreVariant::TeeSimulatorV4 => {
            crate::engine::write_patch_dates(&dates.system, &dates.boot, &dates.vendor)
        }
        TrickyStoreVariant::James => {
            crate::platform::fs::ensure_dir(Path::new(TS_DIR))?;
            write_james(dates)
        }
        TrickyStoreVariant::Standard => {
            crate::platform::fs::ensure_dir(Path::new(TS_DIR))?;
            write_standard(dates)
        }
        TrickyStoreVariant::Legacy => write_legacy(dates),
    }
}

fn write_james(dates: &PatchDates) -> Result<()> {
    let path = Path::new(DEVCONFIG_TOML);
    let content = if path.exists() {
        std::fs::read_to_string(path)?
    } else {
        String::new()
    };

    let new_line = format!("securityPatch = \"{}\"", dates.system);

    let updated = if content.contains("securityPatch") {
        content
            .lines()
            .map(|line| {
                if line.trim_start().starts_with("securityPatch") {
                    new_line.as_str()
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else if content.contains("[deviceProps]") {
        content
            .lines()
            .flat_map(|line| {
                if line.trim() == "[deviceProps]" {
                    vec![line, &new_line as &str]
                } else {
                    vec![line]
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else if content.is_empty() {
        new_line
    } else {
        format!("{content}\n{new_line}")
    };

    atomic_write(path, updated.as_bytes()).context("failed to write devconfig.toml")?;
    info!("wrote security patch to devconfig.toml: {}", dates.system);
    Ok(())
}

fn write_standard(dates: &PatchDates) -> Result<()> {
    let content = format!(
        "system={}\nboot={}\nvendor={}\n",
        dates.system, dates.boot, dates.vendor
    );
    atomic_write(Path::new(SECURITY_PATCH_FILE), content.as_bytes())
        .context("failed to write security_patch.txt")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ =
            std::fs::set_permissions(SECURITY_PATCH_FILE, std::fs::Permissions::from_mode(0o644));
    }

    info!("wrote security patch to security_patch.txt");
    Ok(())
}

fn write_legacy(dates: &PatchDates) -> Result<()> {
    if let Err(e) = prop_set("ro.build.version.security_patch", &dates.system) {
        warn!("resetprop-rs system patch failed: {e}");
    }
    if !dates.vendor.is_empty() {
        if let Err(e) = prop_set("ro.vendor.build.security_patch", &dates.vendor) {
            warn!("resetprop-rs vendor patch failed: {e}");
        }
    }
    info!("applied security patch via resetprop-rs");
    Ok(())
}

fn extract_version_code(prop_content: &str) -> Option<u32> {
    prop_content
        .lines()
        .find(|l| l.starts_with("versionCode="))
        .and_then(|l| l.strip_prefix("versionCode="))
        .and_then(|v| v.trim().parse().ok())
}
