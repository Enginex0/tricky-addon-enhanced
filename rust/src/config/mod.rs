pub mod migrate;

use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

pub static SELF_WRITE: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub keybox: KeyboxConfig,
    pub security_patch: SecurityPatchConfig,
    pub automation: AutomationConfig,
    pub health: HealthConfig,
    pub status: StatusConfig,
    pub vbhash: VbhashConfig,
    pub conflict: ConflictConfig,
    pub props: PropsConfig,
    pub region: RegionConfig,
    pub logging: LoggingConfig,
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub module_id: String,
    pub teesim_profile: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            module_id: "TA_enhanced".into(),
            teesim_profile: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyboxConfig {
    pub enabled: bool,
    pub interval: u32,
    pub source: String,
    pub custom_url: String,
    pub boot_retries: u32,
    pub retry_delay: u32,
}

impl Default for KeyboxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: 300,
            source: "yurikey".into(),
            custom_url: String::new(),
            boot_retries: 10,
            retry_delay: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityPatchConfig {
    pub auto_update: bool,
    pub interval: u32,
    pub custom_date: String,
    pub boot_retries: u32,
}

impl Default for SecurityPatchConfig {
    fn default() -> Self {
        Self {
            auto_update: true,
            interval: 86400,
            custom_date: String::new(),
            boot_retries: 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AutomationConfig {
    pub enabled: bool,
    pub interval: u32,
    pub use_inotify: bool,
    pub exclude_list: Vec<String>,
    pub merge_denylist: bool,
}

impl Default for AutomationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: 10,
            use_inotify: true,
            exclude_list: Vec::new(),
            merge_denylist: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HealthConfig {
    pub enabled: bool,
    pub interval: u32,
    pub grace_period: u32,
    pub max_restarts: u32,
    pub backoff_init: u32,
    pub backoff_cap: u32,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: 10,
            grace_period: 5,
            max_restarts: 10,
            backoff_init: 20,
            backoff_cap: 300,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StatusConfig {
    pub enabled: bool,
    pub interval: u32,
    pub emoji: bool,
}

impl Default for StatusConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: 30,
            emoji: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VbhashConfig {
    pub enabled: bool,
}

impl Default for VbhashConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConflictConfig {
    pub enabled: bool,
    pub auto_remove: bool,
}

impl Default for ConflictConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_remove: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PropsConfig {
    pub custom_props: Vec<[String; 2]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RegionConfig {
    pub enabled: bool,
    pub hwc: String,
    pub hwcountry: String,
    pub mod_device: String,
    pub hardware_sku: String,
}

impl Default for RegionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hwc: String::new(),
            hwcountry: String::new(),
            mod_device: String::new(),
            hardware_sku: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    pub level: String,
    pub max_size_mb: u32,
    pub max_files: u32,
    pub log_dir: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
            max_size_mb: 2,
            max_files: 3,
            log_dir: "/data/adb/tricky_store/ta-enhanced/logs".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub language: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            language: "en".into(),
        }
    }
}

pub const DEFAULT_CONFIG_PATH: &str = "/data/adb/tricky_store/ta-enhanced/config.toml";

const SUPPORTED_LANGS: &[&str] = &[
    "ar", "az", "bn", "de", "el", "en", "es-ES", "fa", "fr", "id", "it", "ja", "ko", "pl", "pt-BR",
    "ru", "th", "tl", "tr", "uk", "vi", "zh-CN", "zh-TW",
];

fn is_supported_lang(code: &str) -> bool {
    SUPPORTED_LANGS.contains(&code)
}

fn parse_bool(s: &str) -> anyhow::Result<bool> {
    match s {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(anyhow!("invalid boolean: {}", s)),
    }
}

const ALL_KEYS: &[&str] = &[
    "general.module_id",
    "general.teesim_profile",
    "keybox.enabled",
    "keybox.interval",
    "keybox.source",
    "keybox.custom_url",
    "keybox.boot_retries",
    "keybox.retry_delay",
    "security_patch.auto_update",
    "security_patch.interval",
    "security_patch.custom_date",
    "security_patch.boot_retries",
    "automation.enabled",
    "automation.interval",
    "automation.use_inotify",
    "automation.exclude_list",
    "automation.merge_denylist",
    "health.enabled",
    "health.interval",
    "health.grace_period",
    "health.max_restarts",
    "health.backoff_init",
    "health.backoff_cap",
    "status.enabled",
    "status.interval",
    "status.emoji",
    "vbhash.enabled",
    "conflict.enabled",
    "conflict.auto_remove",
    "region.enabled",
    "region.hwc",
    "region.hwcountry",
    "region.mod_device",
    "region.hardware_sku",
    "logging.level",
    "logging.max_size_mb",
    "logging.max_files",
    "logging.log_dir",
    "ui.language",
];

impl Config {
    pub fn load(path: Option<&Path>) -> anyhow::Result<Self> {
        let path = path.unwrap_or(Path::new(DEFAULT_CONFIG_PATH));
        if !path.exists() {
            return Ok(Self::default());
        }
        let (config, warnings) = Self::read_validated(path)?;
        for w in &warnings {
            tracing::warn!("{}", w);
        }
        if !warnings.is_empty() {
            SELF_WRITE.store(true, Ordering::Relaxed);
            config.save(Some(path))?;
        }
        Ok(config)
    }

    fn read_validated(path: &Path) -> anyhow::Result<(Self, Vec<String>)> {
        let content = std::fs::read_to_string(path)?;
        let mut config: Config = toml::from_str(&content)?;
        let warnings = config.validate();
        Ok((config, warnings))
    }

    pub fn save(&self, path: Option<&Path>) -> anyhow::Result<()> {
        let path = path.unwrap_or(Path::new(DEFAULT_CONFIG_PATH));
        let content = toml::to_string_pretty(self)?;
        let tmp = path.with_extension("toml.tmp");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&tmp, &content)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn backup(path: Option<&Path>) -> anyhow::Result<()> {
        let path = path.unwrap_or(Path::new(DEFAULT_CONFIG_PATH));
        if path.exists() {
            let bak = path.with_extension("toml.bak");
            std::fs::copy(path, bak)?;
        }
        Ok(())
    }

    pub fn init(path: &Path) -> anyhow::Result<()> {
        if path.exists() {
            return Ok(());
        }
        let config = Self::default();
        config.save(Some(path))?;
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<String> {
        match key {
            "general.module_id" => Some(self.general.module_id.clone()),
            "general.teesim_profile" => Some(self.general.teesim_profile.clone()),
            "keybox.enabled" => Some(self.keybox.enabled.to_string()),
            "keybox.interval" => Some(self.keybox.interval.to_string()),
            "keybox.source" => Some(self.keybox.source.clone()),
            "keybox.custom_url" => Some(self.keybox.custom_url.clone()),
            "keybox.boot_retries" => Some(self.keybox.boot_retries.to_string()),
            "keybox.retry_delay" => Some(self.keybox.retry_delay.to_string()),
            "security_patch.auto_update" => Some(self.security_patch.auto_update.to_string()),
            "security_patch.interval" => Some(self.security_patch.interval.to_string()),
            "security_patch.custom_date" => Some(self.security_patch.custom_date.clone()),
            "security_patch.boot_retries" => Some(self.security_patch.boot_retries.to_string()),
            "automation.enabled" => Some(self.automation.enabled.to_string()),
            "automation.interval" => Some(self.automation.interval.to_string()),
            "automation.use_inotify" => Some(self.automation.use_inotify.to_string()),
            "automation.exclude_list" => Some(self.automation.exclude_list.join(",")),
            "automation.merge_denylist" => Some(self.automation.merge_denylist.to_string()),
            "health.enabled" => Some(self.health.enabled.to_string()),
            "health.interval" => Some(self.health.interval.to_string()),
            "health.grace_period" => Some(self.health.grace_period.to_string()),
            "health.max_restarts" => Some(self.health.max_restarts.to_string()),
            "health.backoff_init" => Some(self.health.backoff_init.to_string()),
            "health.backoff_cap" => Some(self.health.backoff_cap.to_string()),
            "status.enabled" => Some(self.status.enabled.to_string()),
            "status.interval" => Some(self.status.interval.to_string()),
            "status.emoji" => Some(self.status.emoji.to_string()),
            "vbhash.enabled" => Some(self.vbhash.enabled.to_string()),
            "conflict.enabled" => Some(self.conflict.enabled.to_string()),
            "conflict.auto_remove" => Some(self.conflict.auto_remove.to_string()),
            "region.enabled" => Some(self.region.enabled.to_string()),
            "region.hwc" => Some(self.region.hwc.clone()),
            "region.hwcountry" => Some(self.region.hwcountry.clone()),
            "region.mod_device" => Some(self.region.mod_device.clone()),
            "region.hardware_sku" => Some(self.region.hardware_sku.clone()),
            "logging.level" => Some(self.logging.level.clone()),
            "logging.max_size_mb" => Some(self.logging.max_size_mb.to_string()),
            "logging.max_files" => Some(self.logging.max_files.to_string()),
            "logging.log_dir" => Some(self.logging.log_dir.clone()),
            "ui.language" => Some(self.ui.language.clone()),
            _ => None,
        }
    }

    pub fn set(&mut self, key: &str, value: &str) -> anyhow::Result<()> {
        match key {
            "general.teesim_profile" => {
                let value = value.trim();
                if !value.is_empty()
                    && (value.len() > 32
                        || !value
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                {
                    return Err(anyhow!(
                        "TEESimulator profile must be 1-32 letters, digits, '-' or '_'"
                    ));
                }
                self.general.teesim_profile = value.to_string();
            }
            "keybox.enabled" => self.keybox.enabled = parse_bool(value)?,
            "keybox.interval" => self.keybox.interval = value.parse()?,
            "keybox.source" => self.keybox.source = value.to_string(),
            "keybox.custom_url" => self.keybox.custom_url = value.to_string(),
            "keybox.boot_retries" => self.keybox.boot_retries = value.parse()?,
            "keybox.retry_delay" => self.keybox.retry_delay = value.parse()?,
            "security_patch.auto_update" => self.security_patch.auto_update = parse_bool(value)?,
            "security_patch.interval" => self.security_patch.interval = value.parse()?,
            "security_patch.custom_date" => self.security_patch.custom_date = value.to_string(),
            "security_patch.boot_retries" => self.security_patch.boot_retries = value.parse()?,
            "automation.enabled" => self.automation.enabled = parse_bool(value)?,
            "automation.interval" => self.automation.interval = value.parse()?,
            "automation.use_inotify" => self.automation.use_inotify = parse_bool(value)?,
            "automation.merge_denylist" => self.automation.merge_denylist = parse_bool(value)?,
            "automation.exclude_list" => {
                self.automation.exclude_list = value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            "health.enabled" => self.health.enabled = parse_bool(value)?,
            "health.interval" => self.health.interval = value.parse()?,
            "health.grace_period" => self.health.grace_period = value.parse()?,
            "health.max_restarts" => self.health.max_restarts = value.parse()?,
            "health.backoff_init" => self.health.backoff_init = value.parse()?,
            "health.backoff_cap" => self.health.backoff_cap = value.parse()?,
            "status.enabled" => self.status.enabled = parse_bool(value)?,
            "status.interval" => self.status.interval = value.parse()?,
            "status.emoji" => self.status.emoji = parse_bool(value)?,
            "vbhash.enabled" => self.vbhash.enabled = parse_bool(value)?,
            "conflict.enabled" => self.conflict.enabled = parse_bool(value)?,
            "conflict.auto_remove" => self.conflict.auto_remove = parse_bool(value)?,
            "region.enabled" => self.region.enabled = parse_bool(value)?,
            "region.hwc" => self.region.hwc = value.to_string(),
            "region.hwcountry" => self.region.hwcountry = value.to_string(),
            "region.mod_device" => self.region.mod_device = value.to_string(),
            "region.hardware_sku" => self.region.hardware_sku = value.to_string(),
            "logging.level" => self.logging.level = value.to_string(),
            "logging.max_size_mb" => self.logging.max_size_mb = value.parse()?,
            "logging.max_files" => self.logging.max_files = value.parse()?,
            "logging.log_dir" => {
                if !value.starts_with("/data/adb/") {
                    return Err(anyhow!("log_dir must be under /data/adb/"));
                }
                self.logging.log_dir = value.to_string();
            }
            "ui.language" => self.ui.language = value.to_string(),
            _ => return Err(anyhow!("unknown config key: {}", key)),
        }
        let _warnings = self.validate();
        Ok(())
    }

    pub fn validate(&mut self) -> Vec<String> {
        let mut warnings = Vec::new();

        macro_rules! clamp_min {
            ($field:expr, $min:expr, $name:expr) => {
                if $field < $min {
                    warnings.push(format!(
                        "{}: clamped {} -> {} (minimum)",
                        $name, $field, $min
                    ));
                    $field = $min;
                }
            };
        }

        clamp_min!(self.keybox.interval, 60, "keybox.interval");
        clamp_min!(
            self.security_patch.interval,
            3600,
            "security_patch.interval"
        );
        clamp_min!(self.automation.interval, 5, "automation.interval");
        clamp_min!(self.health.interval, 5, "health.interval");
        clamp_min!(self.status.interval, 10, "status.interval");

        self.keybox.boot_retries = self.keybox.boot_retries.clamp(1, 30);
        self.keybox.retry_delay = self.keybox.retry_delay.clamp(1, 30);
        self.security_patch.boot_retries = self.security_patch.boot_retries.clamp(1, 30);
        self.health.grace_period = self.health.grace_period.max(1);
        self.health.max_restarts = self.health.max_restarts.max(1);
        self.health.backoff_init = self.health.backoff_init.max(5);
        self.health.backoff_cap = self.health.backoff_cap.max(30);
        self.logging.max_size_mb = self.logging.max_size_mb.clamp(1, 50);
        self.logging.max_files = self.logging.max_files.clamp(1, 10);

        match self.keybox.source.as_str() {
            "yurikey" | "upstream" | "custom" => {}
            other => {
                warnings.push(format!(
                    "keybox.source: legacy value '{}' migrated to 'yurikey'",
                    other
                ));
                self.keybox.source = "yurikey".into();
            }
        }
        match self.logging.level.as_str() {
            "error" | "warn" | "info" | "debug" | "trace" => {}
            _ => self.logging.level = "info".into(),
        }
        if !is_supported_lang(&self.ui.language) {
            let prev = std::mem::take(&mut self.ui.language);
            warnings.push(format!(
                "ui.language: unsupported value '{}' reset to 'en'",
                prev
            ));
            self.ui.language = "en".into();
        }
        if !self.logging.log_dir.starts_with("/data/adb/") {
            warnings
                .push("logging.log_dir: reset to default (must be under /data/adb/)".to_string());
            self.logging.log_dir = "/data/adb/tricky_store/ta-enhanced/logs".into();
        }

        warnings
    }

    pub fn list(&self) {
        for key in ALL_KEYS {
            if let Some(val) = self.get(key) {
                println!("{key} = {val}");
            }
        }
    }

    pub fn dump(&self, json: bool) -> anyhow::Result<()> {
        if json {
            println!("{}", serde_json::to_string_pretty(self)?);
        } else {
            println!("{}", toml::to_string_pretty(self)?);
        }
        Ok(())
    }

    pub fn defaults() -> anyhow::Result<()> {
        let def = Self::default();
        println!("{}", toml::to_string_pretty(&def)?);
        Ok(())
    }

    pub fn restore(path: Option<&Path>) -> anyhow::Result<()> {
        let path = path.unwrap_or(Path::new(DEFAULT_CONFIG_PATH));
        let bak = path.with_extension("toml.bak");
        if !bak.exists() {
            anyhow::bail!("no backup found at {}", bak.display());
        }
        std::fs::copy(&bak, path)?;
        Ok(())
    }
}

pub fn handle_config(action: crate::cli::ConfigAction, cfg: &Config) -> anyhow::Result<()> {
    use crate::cli::ConfigAction;
    match action {
        ConfigAction::Get { key } => {
            match cfg.get(&key) {
                Some(val) => println!("{val}"),
                None => anyhow::bail!("unknown config key: {key}"),
            }
            Ok(())
        }
        ConfigAction::Set { key, value } => {
            if key == "general.teesim_profile" {
                crate::engine::validate_profile_choice(value.trim())?;
            }
            let mut current = Config::load(None)?;
            current.set(&key, &value)?;
            Config::backup(None)?;
            current.save(None)
        }
        ConfigAction::Migrate => {
            let ini_path = std::path::Path::new("/data/adb/tricky_store/enhanced.conf");
            let toml_path = std::path::Path::new(DEFAULT_CONFIG_PATH);
            migrate::migrate_ini_to_toml(ini_path, toml_path)
        }
        ConfigAction::List => {
            cfg.list();
            Ok(())
        }
        ConfigAction::Init { automation } => {
            let path = std::path::Path::new(DEFAULT_CONFIG_PATH);
            Config::init(path)?;
            if let Some(auto_val) = automation {
                let mut cfg = Config::load(Some(path))?;
                cfg.set("automation.enabled", &auto_val)?;
                cfg.save(Some(path))?;
            }
            Ok(())
        }
        ConfigAction::Dump { json } => cfg.dump(json),
        ConfigAction::Defaults => Config::defaults(),
        ConfigAction::Restore => Config::restore(None),
        ConfigAction::PropsCustom => {
            for pair in &cfg.props.custom_props {
                if pair.len() == 2 && !pair[0].is_empty() {
                    println!("{}\t{}", pair[0], pair[1]);
                }
            }
            Ok(())
        }
    }
}
