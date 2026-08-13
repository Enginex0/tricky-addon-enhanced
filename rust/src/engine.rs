use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::platform::fs::atomic_write;

pub const TRICKY_MODULE: &str = "/data/adb/modules/tricky_store";
pub const TRICKY_MODULE_HIDDEN: &str = "/data/adb/modules/.tricky_store";
pub const TRICKY_DATA: &str = "/data/adb/tricky_store";
pub const TEESIM_MODULE: &str = "/data/adb/modules/teesim";
pub const TEESIM_MODULE_UPDATE: &str = "/data/adb/modules_update/teesim";
pub const TEESIM_DATA: &str = "/data/adb/teesim";
pub const TARGET_MIRROR: &str = "/data/adb/tricky_store/target.txt";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    TrickyStore,
    TeeSimulatorV4,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileStatus {
    pub available: bool,
    pub profiles: Vec<String>,
    pub selected: Option<String>,
    pub automatic: bool,
}

impl Engine {
    pub fn detect() -> Self {
        if module_is_present(Path::new(TEESIM_MODULE))
            || module_is_present(Path::new(TEESIM_MODULE_UPDATE))
        {
            Self::TeeSimulatorV4
        } else {
            Self::TrickyStore
        }
    }

    pub fn keybox_path(self) -> Result<PathBuf> {
        match self {
            Self::TrickyStore => Ok(PathBuf::from(TRICKY_DATA).join("keybox.xml")),
            Self::TeeSimulatorV4 => teesim_keybox_path(),
        }
    }

    pub fn module_dir(self) -> Option<PathBuf> {
        let candidates: &[&str] = match self {
            Self::TrickyStore => &[TRICKY_MODULE, TRICKY_MODULE_HIDDEN],
            Self::TeeSimulatorV4 => &[TEESIM_MODULE, TEESIM_MODULE_UPDATE],
        };
        candidates
            .iter()
            .map(PathBuf::from)
            .find(|path| match self {
                Self::TeeSimulatorV4 => module_is_present(path),
                Self::TrickyStore => path.is_dir() && !path.join("remove").exists(),
            })
    }

    pub fn is_enabled(self) -> bool {
        self.module_dir()
            .map(|path| !path.join("disable").exists())
            .unwrap_or(false)
    }
}

fn module_is_present(path: &Path) -> bool {
    if !path.is_dir() || path.join("remove").exists() {
        return false;
    }
    std::fs::read_to_string(path.join("module.prop"))
        .map(|content| content.lines().any(|line| line == "id=teesim"))
        .unwrap_or(false)
}

pub fn read_targets() -> Result<Vec<String>> {
    match Engine::detect() {
        Engine::TrickyStore => read_tricky_targets(),
        Engine::TeeSimulatorV4 => {
            let config = read_teesim_config()?;
            let profile = selected_profile(&config)?;
            Ok(profile
                .get("apps")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect())
        }
    }
}

fn read_tricky_targets() -> Result<Vec<String>> {
    let path = Path::new(TARGET_MIRROR);
    if !path.exists() {
        return Ok(Vec::new());
    }
    Ok(std::fs::read_to_string(path)?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect())
}

pub fn write_targets(entries: &[String]) -> Result<()> {
    match Engine::detect() {
        Engine::TrickyStore => {
            let entries: Vec<String> = entries
                .iter()
                .map(|entry| entry.trim())
                .filter(|entry| !entry.is_empty())
                .map(str::to_owned)
                .collect();
            write_target_mirror(&entries)
        }
        Engine::TeeSimulatorV4 => {
            let normalized = normalize_targets(entries);
            mutate_teesim_config(|config| update_profile_apps(config, &normalized))?;
            write_target_mirror(&normalized)
        }
    }
}

pub fn export_target_mirror() -> Result<()> {
    if Engine::detect() == Engine::TrickyStore {
        return Ok(());
    }
    let targets = read_targets()?;
    write_target_mirror(&targets)
}

pub fn import_target_mirror() -> Result<()> {
    if Engine::detect() == Engine::TrickyStore {
        return Ok(());
    }
    let targets = read_tricky_targets()?;
    write_targets(&targets)
}

fn normalize_targets(entries: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    entries
        .iter()
        .map(|entry| entry.trim().trim_end_matches(['!', '?']))
        .filter(|entry| !entry.is_empty() && !entry.starts_with('#'))
        .filter(|entry| seen.insert((*entry).to_owned()))
        .map(str::to_owned)
        .collect()
}

pub(crate) fn write_target_mirror(entries: &[String]) -> Result<()> {
    let mut content = entries.join("\n");
    if !content.is_empty() {
        content.push('\n');
    }
    atomic_write(Path::new(TARGET_MIRROR), content.as_bytes())
}

pub fn read_patch_dates() -> Result<(String, String, String)> {
    if Engine::detect() == Engine::TrickyStore {
        let content = std::fs::read_to_string(Path::new(TRICKY_DATA).join("security_patch.txt"))?;
        return Ok(parse_tricky_patch_dates(&content));
    }

    let config = read_teesim_config()?;
    let patch = selected_profile(&config)?
        .get("patchLevel")
        .and_then(Value::as_object);
    Ok((
        patch_value(patch, "system"),
        patch_value(patch, "boot"),
        patch_value(patch, "vendor"),
    ))
}

pub fn write_patch_dates(system: &str, boot: &str, vendor: &str) -> Result<()> {
    let system = normalize_patch_value(system)?;
    let boot = normalize_patch_value(boot)?;
    let vendor = normalize_patch_value(vendor)?;
    mutate_teesim_config(|config| {
        let profile = selected_profile_mut(config)?;
        let patch = profile
            .entry("patchLevel")
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .context("TEESimulator profile patchLevel must be an object")?;
        patch.insert("system".into(), Value::String(system));
        patch.insert("boot".into(), Value::String(boot));
        patch.insert("vendor".into(), Value::String(vendor));
        Ok(())
    })
}

pub fn import_legacy_patch() -> Result<()> {
    let path = Path::new(TRICKY_DATA).join("security_patch.txt");
    if !path.exists() {
        return write_patch_dates("no", "no", "no");
    }
    let (mut system, mut boot, mut vendor) =
        parse_tricky_patch_dates(&std::fs::read_to_string(path)?);
    for value in [&mut system, &mut boot, &mut vendor] {
        if value.is_empty() {
            *value = "no".to_owned();
        }
    }
    write_patch_dates(&system, &boot, &vendor)
}

fn normalize_patch_value(value: &str) -> Result<String> {
    let value = value.trim();
    let normalized = match value {
        "prop" => "system_property".to_owned(),
        _ if valid_patch_value(value) => value.to_owned(),
        _ if value.len() == 8 && value.chars().all(|c| c.is_ascii_digit()) => {
            format!("{}-{}-{}", &value[..4], &value[4..6], &value[6..8])
        }
        _ if value.len() == 6 && value.chars().all(|c| c.is_ascii_digit()) => {
            format!("{}-{}", &value[..4], &value[4..6])
        }
        _ => bail!("invalid TEESimulator patch level value: {value}"),
    };
    if !valid_patch_value(&normalized) {
        bail!("invalid TEESimulator patch level value: {value}");
    }
    Ok(normalized)
}

fn valid_patch_value(value: &str) -> bool {
    if matches!(value, "today" | "no" | "harvested" | "system_property") {
        return true;
    }
    let parts: Vec<&str> = value.split('-').collect();
    if !(2..=3).contains(&parts.len()) {
        return false;
    }
    let year =
        parts[0] == "YYYY" || (parts[0].len() == 4 && parts[0].chars().all(|c| c.is_ascii_digit()));
    let month = parts[1] == "MM"
        || (parts[1].len() == 2
            && parts[1]
                .parse::<u8>()
                .is_ok_and(|month| (1..=12).contains(&month)));
    let day = parts.len() == 2 || parts[2] == "DD" || valid_patch_day(parts[2]);
    year && month && day
}

fn valid_patch_day(value: &str) -> bool {
    value.len() == 2
        && value.chars().all(|c| c.is_ascii_digit())
        && value.parse::<u8>().is_ok_and(|day| (1..=31).contains(&day))
}

fn patch_value(patch: Option<&Map<String, Value>>, key: &str) -> String {
    patch
        .and_then(|values| values.get(key))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn parse_tricky_patch_dates(content: &str) -> (String, String, String) {
    let find = |key: &str| {
        content
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .map(str::trim)
            .unwrap_or_default()
            .to_owned()
    };
    let all = find("all=");
    if !all.is_empty() {
        return (all.clone(), all.clone(), all);
    }
    (find("system="), find("boot="), find("vendor="))
}

fn teesim_config_path() -> PathBuf {
    PathBuf::from(TEESIM_DATA).join("config.json")
}

fn teesim_keybox_path() -> Result<PathBuf> {
    let config = read_teesim_config()?;
    let keybox = selected_profile(&config)?
        .get("keybox")
        .and_then(Value::as_str)
        .context("TEESimulator selected profile has no keybox")?;
    Ok(PathBuf::from(TEESIM_DATA).join(keybox))
}

fn read_teesim_config() -> Result<Value> {
    let path = teesim_config_path();
    if !path.exists() {
        bail!("TEESimulator config does not exist at {}", path.display());
    }
    let content =
        std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    parse_teesim_config(&content, &path)
}

pub fn ensure_profile_selected() -> Result<()> {
    if Engine::detect() == Engine::TeeSimulatorV4 {
        let config = read_teesim_config()?;
        selected_profile_name(&config)?;
    }
    Ok(())
}

pub fn profile_status(configured: &str) -> Result<ProfileStatus> {
    if Engine::detect() != Engine::TeeSimulatorV4 {
        return Ok(ProfileStatus {
            available: false,
            profiles: Vec::new(),
            selected: None,
            automatic: false,
        });
    }
    profile_status_for(&read_teesim_config()?, configured)
}

pub fn validate_profile_choice(configured: &str) -> Result<()> {
    if configured.is_empty() || Engine::detect() != Engine::TeeSimulatorV4 {
        return Ok(());
    }
    let config = read_teesim_config()?;
    if !profiles(&config).is_some_and(|profiles| profiles.contains_key(configured)) {
        bail!("TEESimulator profile does not exist: {configured}");
    }
    Ok(())
}

fn profile_status_for(config: &Value, configured: &str) -> Result<ProfileStatus> {
    validate_teesim_config(config)?;
    let profiles = profiles(config).context("TEESimulator profiles must be an object")?;
    let names: Vec<String> = profiles.keys().cloned().collect();
    let automatic = names.len() == 1;
    let selected = if automatic {
        names.first().cloned()
    } else if profiles.contains_key(configured) {
        Some(configured.to_owned())
    } else {
        None
    };
    Ok(ProfileStatus {
        available: true,
        profiles: names,
        selected,
        automatic,
    })
}

fn parse_teesim_config(content: &[u8], path: &Path) -> Result<Value> {
    let config: Value = serde_json::from_slice(content)
        .with_context(|| format!("invalid TEESimulator config at {}", path.display()))?;
    validate_teesim_config(&config)?;
    Ok(config)
}

fn validate_teesim_config(config: &Value) -> Result<()> {
    config
        .as_object()
        .context("TEESimulator config root must be an object")?;
    if config.get("version").and_then(Value::as_u64) != Some(1) {
        bail!("unsupported TEESimulator config schema (expected version 1)");
    }
    let profiles = profiles(config).context("TEESimulator profiles must be an object")?;
    if profiles.is_empty() {
        bail!("TEESimulator profiles must not be empty");
    }
    let mut claimed_apps: HashMap<String, &str> = HashMap::new();
    let mut auto_include_profiles = 0;
    for (name, profile) in profiles {
        if name.is_empty()
            || name.len() > 32
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            bail!("TEESimulator profile name is invalid: {name}");
        }
        let profile = profile
            .as_object()
            .with_context(|| format!("TEESimulator profile {name} must be an object"))?;
        let keybox = profile
            .get("keybox")
            .and_then(Value::as_str)
            .with_context(|| format!("TEESimulator profile {name} has no keybox"))?;
        if !valid_keybox_name(keybox) {
            bail!("TEESimulator profile {name} keybox must be an XML filename");
        }
        let empty_apps = Vec::new();
        let apps = match profile.get("apps") {
            Some(value) => value
                .as_array()
                .with_context(|| format!("TEESimulator profile {name} apps must be an array"))?,
            None => &empty_apps,
        };
        let auto_include = profile
            .get("autoIncludeNewApps")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if profile.contains_key("autoIncludeNewApps") && !profile["autoIncludeNewApps"].is_boolean()
        {
            bail!("TEESimulator profile {name} autoIncludeNewApps must be a boolean");
        }
        if let Some(mode) = profile.get("mode") {
            if !matches!(mode.as_str(), Some("patch" | "generation")) {
                bail!("TEESimulator profile {name} mode must be patch or generation");
            }
        }
        if let Some(patch) = profile.get("patchLevel") {
            let patch = patch.as_object().with_context(|| {
                format!("TEESimulator profile {name} patchLevel must be an object")
            })?;
            for field in ["system", "vendor", "boot"] {
                if let Some(value) = patch.get(field) {
                    let value = value.as_str().with_context(|| {
                        format!("TEESimulator profile {name} patchLevel.{field} must be a string")
                    })?;
                    if !value.is_empty() && !valid_patch_value(value) {
                        bail!(
                            "TEESimulator profile {name} has invalid patchLevel.{field}: {value}"
                        );
                    }
                }
            }
        }
        if auto_include {
            auto_include_profiles += 1;
            if auto_include_profiles > 1 {
                bail!("only one TEESimulator profile may auto-include new apps");
            }
        } else if apps.is_empty() {
            bail!("TEESimulator profile {name} has no apps");
        }
        for app in apps {
            let app = app.as_str().with_context(|| {
                format!("TEESimulator profile {name} apps must contain strings")
            })?;
            if !valid_app_entry(app) {
                bail!("TEESimulator profile {name} has invalid app entry: {app}");
            }
            if let Some(owner) = claimed_apps.insert(app.to_owned(), name) {
                if owner != name {
                    bail!("TEESimulator app entry appears in profiles {owner} and {name}: {app}");
                }
            }
        }
    }
    validate_effective_uid_ownership(profiles)?;
    Ok(())
}

fn package_uids() -> HashMap<String, u32> {
    crate::platform::packages::list_with_uids().unwrap_or_default()
}

fn effective_uid(entry: &str, packages: &HashMap<String, u32>) -> Option<u32> {
    entry
        .strip_prefix("uid:")
        .and_then(|uid| uid.parse().ok())
        .or_else(|| packages.get(entry).copied())
}

fn validate_effective_uid_ownership(profiles: &Map<String, Value>) -> Result<()> {
    let packages = package_uids();
    let mut owners: HashMap<u32, &str> = HashMap::new();
    for (name, profile) in profiles {
        for app in profile
            .get("apps")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let Some(uid) = effective_uid(app, &packages) else {
                continue;
            };
            if let Some(owner) = owners.insert(uid, name) {
                if owner != name {
                    bail!("TEESimulator UID {uid} is claimed by profiles {owner} and {name}");
                }
            }
        }
    }
    Ok(())
}

fn valid_keybox_name(value: &str) -> bool {
    value.ends_with(".xml")
        && value.len() > 4
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn valid_app_entry(value: &str) -> bool {
    value
        .strip_prefix("uid:")
        .is_some_and(|uid| uid.parse::<i32>().is_ok_and(|uid| uid >= 0))
        || (!value.is_empty()
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_')))
}

fn profiles(config: &Value) -> Option<&Map<String, Value>> {
    config.get("profiles").and_then(Value::as_object)
}

fn selected_profile_name(config: &Value) -> Result<&str> {
    let profiles = profiles(config).context("TEESimulator profiles must be an object")?;
    if profiles.len() == 1 {
        return profiles
            .keys()
            .next()
            .map(String::as_str)
            .context("TEESimulator profiles must not be empty");
    }
    let configured = crate::config::Config::load(None)?.general.teesim_profile;
    if configured.is_empty() {
        bail!(
            "TEESimulator has multiple profiles; select one in the addon WebUI or run: ta-enhanced automation select-profile <name>"
        );
    }
    profiles
        .get_key_value(&configured)
        .map(|(name, _)| name.as_str())
        .with_context(|| format!("configured TEESimulator profile does not exist: {configured}"))
}

fn selected_profile(config: &Value) -> Result<&Map<String, Value>> {
    let name = selected_profile_name(config)?;
    profiles(config)
        .and_then(|profiles| profiles.get(name))
        .and_then(Value::as_object)
        .context("selected TEESimulator profile must be an object")
}

fn selected_profile_mut(config: &mut Value) -> Result<&mut Map<String, Value>> {
    let selected = selected_profile_name(config)?.to_owned();
    let profiles = config
        .get_mut("profiles")
        .and_then(Value::as_object_mut)
        .context("TEESimulator profiles must be an object")?;
    profiles
        .get_mut(&selected)
        .and_then(Value::as_object_mut)
        .context("TEESimulator profile must be an object")
}

fn update_profile_apps(config: &mut Value, entries: &[String]) -> Result<Vec<String>> {
    let selected = selected_profile_name(config)?.to_owned();
    update_profile_apps_for(config, entries, &selected)
}

fn update_profile_apps_for(
    config: &mut Value,
    entries: &[String],
    selected: &str,
) -> Result<Vec<String>> {
    let occupied: HashSet<String> = profiles(config)
        .into_iter()
        .flat_map(|profiles| profiles.iter())
        .filter(|(name, _)| name.as_str() != selected)
        .flat_map(|(_, profile)| {
            profile
                .get("apps")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    let packages = package_uids();
    let occupied_uids: HashSet<u32> = occupied
        .iter()
        .filter_map(|entry| effective_uid(entry, &packages))
        .collect();
    let effective: Vec<String> = entries
        .iter()
        .filter(|entry| {
            !occupied.contains(entry.as_str())
                && effective_uid(entry, &packages).is_none_or(|uid| !occupied_uids.contains(&uid))
        })
        .cloned()
        .collect();
    if effective.len() != entries.len() {
        let rejected: Vec<&str> = entries
            .iter()
            .filter(|entry| {
                occupied.contains(entry.as_str())
                    || effective_uid(entry, &packages)
                        .is_some_and(|uid| occupied_uids.contains(&uid))
            })
            .map(String::as_str)
            .collect();
        bail!(
            "apps already assigned to another TEESimulator profile: {}",
            rejected.join(", ")
        );
    }
    let apps = effective.iter().cloned().map(Value::String).collect();
    profiles(config)
        .and_then(|profiles| profiles.get(selected))
        .context("selected TEESimulator profile does not exist")?;
    config["profiles"][selected]["apps"] = Value::Array(apps);
    Ok(effective)
}

fn mutate_teesim_config<T, F>(mutation: F) -> Result<T>
where
    F: FnOnce(&mut Value) -> Result<T>,
{
    let config_path = teesim_config_path();
    let mut config = read_teesim_config()?;
    let result = mutation(&mut config)?;
    validate_teesim_config(&config)?;
    let mut data = serde_json::to_vec_pretty(&config)?;
    data.push(b'\n');
    atomic_write(&config_path, &data)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn updates_only_selected_profile_and_preserves_unknown_fields() {
        let mut config = json!({
            "version": 1,
            "futureField": true,
            "profiles": {
                "default": { "keybox": "keybox.xml", "apps": ["old.app"], "autoIncludeNewApps": true },
                "banking": { "keybox": "bank.xml", "apps": ["reserved.app"] }
            }
        });
        let effective =
            update_profile_apps_for(&mut config, &["new.app".into()], "default").unwrap();

        assert_eq!(config["futureField"], true);
        assert_eq!(config["profiles"]["default"]["autoIncludeNewApps"], true);
        assert_eq!(config["profiles"]["default"]["apps"], json!(["new.app"]));
        assert_eq!(
            config["profiles"]["banking"]["apps"],
            json!(["reserved.app"])
        );
        assert_eq!(effective, vec!["new.app"]);
    }

    #[test]
    fn rejects_apps_owned_by_another_profile() {
        let mut config = json!({
            "version": 1,
            "profiles": {
                "default": { "keybox": "keybox.xml", "apps": ["old.app"], "autoIncludeNewApps": true },
                "banking": { "keybox": "bank.xml", "apps": ["reserved.app"] }
            }
        });

        let error = update_profile_apps_for(
            &mut config,
            &["new.app".into(), "reserved.app".into()],
            "default",
        )
        .unwrap_err();

        assert!(error.to_string().contains("reserved.app"));
        assert_eq!(config["profiles"]["default"]["apps"], json!(["old.app"]));
    }

    #[test]
    fn strips_trickystore_suffixes_for_teesim() {
        assert_eq!(
            normalize_targets(&["one.app!".into(), "two.app?".into(), "one.app".into()]),
            vec!["one.app", "two.app"]
        );
    }

    #[test]
    fn validates_uid_tokens_at_kotlin_int_boundary() {
        assert!(valid_app_entry("uid:2147483647"));
        assert!(!valid_app_entry("uid:2147483648"));
        assert!(!valid_app_entry("uid:999999999999999999999"));
        assert!(!valid_app_entry("uid:-1"));
    }

    #[test]
    fn normalizes_legacy_patch_values() {
        assert_eq!(normalize_patch_value("20250105").unwrap(), "2025-01-05");
        assert_eq!(normalize_patch_value("202501").unwrap(), "2025-01");
        assert_eq!(normalize_patch_value("prop").unwrap(), "system_property");
        assert_eq!(normalize_patch_value("YYYY-MM-05").unwrap(), "YYYY-MM-05");
        assert_eq!(normalize_patch_value("YYYY-MM-DD").unwrap(), "YYYY-MM-DD");
        assert_eq!(normalize_patch_value("YYYY-08").unwrap(), "YYYY-08");
        assert_eq!(normalize_patch_value("2026-MM").unwrap(), "2026-MM");
        assert_eq!(normalize_patch_value("2026-08-DD").unwrap(), "2026-08-DD");
        assert_eq!(normalize_patch_value("YYYY-08-DD").unwrap(), "YYYY-08-DD");
        assert!(normalize_patch_value("2025-13-01").is_err());
        assert!(normalize_patch_value("YYYY-MM-32").is_err());
    }

    #[test]
    fn rejects_malformed_or_profileless_configs() {
        assert!(validate_teesim_config(&json!({ "version": 1, "profiles": [] })).is_err());
        assert!(validate_teesim_config(&json!({
            "version": 1,
            "profiles": { "other": { "keybox": "keybox.xml", "apps": ["com.example"] } }
        }))
        .is_ok());
        assert!(validate_teesim_config(&json!({
            "version": 1,
            "profiles": { "default": { "keybox": "../keybox.xml", "apps": [] } }
        }))
        .is_err());
        assert!(validate_teesim_config(&json!({
            "version": 1,
            "profiles": { "default": {
                "keybox": "keybox.xml", "apps": ["com.example"], "mode": "invalid"
            } }
        }))
        .is_err());
        assert!(validate_teesim_config(&json!({
            "version": 1,
            "profiles": { "default": {
                "keybox": "keybox.xml", "apps": ["com.example"],
                "autoIncludeNewApps": "true"
            } }
        }))
        .is_err());
    }

    #[test]
    fn updates_explicit_profile_when_default_is_absent() {
        let mut config = json!({
            "version": 1,
            "profiles": {
                "pixel": { "keybox": "pixel.xml", "apps": ["old.app"] },
                "banking": { "keybox": "bank.xml", "apps": ["reserved.app"] }
            }
        });

        update_profile_apps_for(&mut config, &["new.app".into()], "pixel").unwrap();

        assert_eq!(config["profiles"]["pixel"]["apps"], json!(["new.app"]));
        assert_eq!(
            config["profiles"]["banking"]["apps"],
            json!(["reserved.app"])
        );
    }

    #[test]
    fn reports_single_profile_as_automatic() {
        let status = profile_status_for(
            &json!({
                "version": 1,
                "profiles": {
                    "default": { "keybox": "keybox.xml", "apps": ["com.example"] }
                }
            }),
            "stale",
        )
        .unwrap();

        assert_eq!(status.profiles, vec!["default"]);
        assert_eq!(status.selected.as_deref(), Some("default"));
        assert!(status.automatic);
    }

    #[test]
    fn reports_multi_profile_selection_and_pending_states() {
        let config = json!({
            "version": 1,
            "profiles": {
                "default": { "keybox": "keybox.xml", "apps": ["com.example"] },
                "banking": { "keybox": "bank.xml", "apps": ["com.bank"] }
            }
        });

        let selected = profile_status_for(&config, "banking").unwrap();
        assert_eq!(selected.selected.as_deref(), Some("banking"));
        assert!(!selected.automatic);

        let pending = profile_status_for(&config, "missing").unwrap();
        assert_eq!(pending.selected, None);
    }

}
