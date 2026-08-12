pub mod generate;
pub mod sources;
pub mod validate;

use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use crate::cli::KeyboxAction;
use crate::config::Config;
use crate::platform::fs::atomic_write;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyboxSource {
    #[default]
    Yurikey,
    Upstream,
    Custom,
}

impl fmt::Display for KeyboxSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Yurikey => write!(f, "yurikey"),
            Self::Upstream => write!(f, "upstream"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

impl FromStr for KeyboxSource {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "yurikey" => Ok(Self::Yurikey),
            "upstream" => Ok(Self::Upstream),
            "custom" => Ok(Self::Custom),
            _ => bail!("unknown keybox source: {s}"),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SourceInfo {
    pub name: String,
    pub active: bool,
}

#[derive(Debug)]
pub struct FetchResult {
    pub source: &'static str,
}

pub fn handle_keybox(action: KeyboxAction, cfg: &Config) -> Result<()> {
    match action {
        KeyboxAction::Fetch => {
            let result = fetch(cfg)?;
            println!("keybox fetched from {}", result.source);
            Ok(())
        }
        KeyboxAction::Validate { path } => {
            use std::io::Write;
            let target = match path {
                Some(path) => PathBuf::from(path),
                None => crate::engine::Engine::detect().keybox_path()?,
            };
            let report = validate::validate_file_full(&target)?;
            let mut stdout = std::io::stdout().lock();
            writeln!(stdout, "{}", serde_json::to_string_pretty(&report)?)?;
            stdout.flush()?;
            if report.ok {
                Ok(())
            } else {
                bail!("keybox validation failed: {}", target.display())
            }
        }
        KeyboxAction::SetCustom { path } => {
            set_custom(Path::new(&path))?;
            println!("custom keybox installed from {path}");
            Ok(())
        }
        KeyboxAction::Sources => {
            let sources = get_sources(cfg);
            let json = serde_json::to_string_pretty(&sources)?;
            println!("{json}");
            Ok(())
        }
        KeyboxAction::Generate => {
            generate::generate_and_install()?;
            println!("device keybox generated");
            Ok(())
        }
        KeyboxAction::Backup => {
            backup()?;
            println!("keybox backed up");
            Ok(())
        }
    }
}

pub fn fetch(config: &Config) -> Result<FetchResult> {
    let preferred = KeyboxSource::from_str(&config.keybox.source).unwrap_or_else(|e| {
        warn!(
            "keybox source {:?} invalid ({e}); defaulting to yurikey",
            config.keybox.source
        );
        KeyboxSource::default()
    });
    let custom_url = &config.keybox.custom_url;

    let order = build_source_order(preferred);
    for source in &order {
        let result = match source {
            KeyboxSource::Yurikey => sources::fetch_yurikey(),
            KeyboxSource::Upstream => sources::fetch_upstream(),
            KeyboxSource::Custom => {
                if custom_url.is_empty() {
                    continue;
                }
                sources::fetch_custom_url(custom_url)
            }
        };

        match result {
            Ok(data) => {
                let report = match validate::validate_full(&data) {
                    Ok(r) => r,
                    Err(e) => {
                        warn!("keybox from {} parse failed: {e}", source);
                        continue;
                    }
                };
                if !report.ok {
                    let summary = report
                        .keys
                        .iter()
                        .filter(|k| !k.ok)
                        .map(|k| format!("{}={}", k.algorithm, k.errors.join(",")))
                        .collect::<Vec<_>>()
                        .join("; ");
                    warn!("keybox from {} rejected: {summary}", source);
                    continue;
                }
                let root_type = report
                    .keys
                    .first()
                    .map(|k| k.root_type.as_snake_case())
                    .unwrap_or("unknown");
                let new_hash = sources::compute_sha256(&data);
                if !install_data(&data, &new_hash)? {
                    info!(
                        "keybox from {} (root={root_type}) identical to installed, skipping",
                        source
                    );
                    return Ok(FetchResult {
                        source: source_label(source),
                    });
                }
                info!("keybox installed from {} (root={root_type})", source);
                return Ok(FetchResult {
                    source: source_label(source),
                });
            }
            Err(e) => {
                warn!("keybox source {} failed: {e}", source);
            }
        }
    }

    if has_valid_existing_keybox() {
        warn!("all remote sources failed, preserving existing keybox");
        return Ok(FetchResult { source: "existing" });
    }

    bail!("all keybox sources failed")
}

pub fn backup() -> Result<()> {
    let target = crate::engine::Engine::detect().keybox_path()?;
    backup_target(&target)
}

fn backup_target(target: &Path) -> Result<()> {
    if !target.exists() {
        bail!("no keybox to backup at {}", target.display());
    }
    let bak = target.with_extension("xml.bak");
    if bak.exists() {
        let bak1 = PathBuf::from(format!("{}.1", bak.display()));
        std::fs::rename(&bak, &bak1).context("failed to rotate backup")?;
    }
    std::fs::copy(&target, &bak).context("failed to create keybox backup")?;
    info!("keybox backed up to {}", bak.display());
    Ok(())
}

pub fn set_custom(path: &Path) -> Result<()> {
    if !path.exists() {
        bail!("custom keybox not found: {}", path.display());
    }
    let data = std::fs::read(path)?;
    let report =
        validate::validate_full(&data).with_context(|| format!("validating {}", path.display()))?;
    if !report.ok {
        let summary = report
            .keys
            .iter()
            .filter(|k| !k.ok)
            .map(|k| {
                format!(
                    "Keybox#{}/Key#{} ({}): {}",
                    k.keybox_index,
                    k.key_index,
                    k.algorithm,
                    k.errors.join("; ")
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");
        bail!("custom keybox failed validation: {summary}");
    }
    install_data(&data, &sources::compute_sha256(&data))?;
    let root_type = report
        .keys
        .first()
        .map(|k| k.root_type.as_snake_case())
        .unwrap_or("unknown");
    info!(
        "custom keybox installed from {} (root={root_type})",
        path.display()
    );
    Ok(())
}

pub fn get_sources(config: &Config) -> Vec<SourceInfo> {
    let active = KeyboxSource::from_str(&config.keybox.source).unwrap_or_else(|e| {
        warn!(
            "keybox source {:?} invalid ({e}); defaulting to yurikey",
            config.keybox.source
        );
        KeyboxSource::default()
    });
    vec![
        SourceInfo {
            name: "yurikey".into(),
            active: active == KeyboxSource::Yurikey,
        },
        SourceInfo {
            name: "upstream".into(),
            active: active == KeyboxSource::Upstream,
        },
        SourceInfo {
            name: "custom".into(),
            active: active == KeyboxSource::Custom,
        },
    ]
}

fn build_source_order(preferred: KeyboxSource) -> Vec<KeyboxSource> {
    let all = [
        KeyboxSource::Yurikey,
        KeyboxSource::Upstream,
        KeyboxSource::Custom,
    ];
    let mut order = vec![preferred];
    for s in all {
        if s != preferred {
            order.push(s);
        }
    }
    order
}

fn install_data(data: &[u8], new_hash: &str) -> Result<bool> {
    let target = crate::engine::Engine::detect().keybox_path()?;
    if !new_hash.is_empty() && current_keybox_hash(&target).as_deref() == Some(new_hash) {
        return Ok(false);
    }
    if target.exists() {
        if let Err(e) = backup_target(&target) {
            error!("backup failed before install: {e}");
            bail!("aborting keybox install: backup failed");
        }
    }
    atomic_write(&target, data).context("failed to write keybox")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600));
    }
    Ok(true)
}

fn current_keybox_hash(target: &Path) -> Option<String> {
    let data = std::fs::read(target).ok()?;
    let hash = sources::compute_sha256(&data);
    if hash.is_empty() {
        None
    } else {
        Some(hash)
    }
}

fn has_valid_existing_keybox() -> bool {
    let Ok(target) = crate::engine::Engine::detect().keybox_path() else {
        return false;
    };
    target.exists() && validate::validate_file(&target).is_ok()
}

fn source_label(s: &KeyboxSource) -> &'static str {
    match s {
        KeyboxSource::Yurikey => "yurikey",
        KeyboxSource::Upstream => "upstream",
        KeyboxSource::Custom => "custom",
    }
}
