use clap::{Parser, Subcommand};

pub mod applist;
pub mod handlers;
pub mod webui_init;

#[derive(Parser)]
#[command(name = "ta-enhanced", version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    #[arg(short, long)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Version,
    Daemon {
        #[arg(long)]
        manager: Option<String>,
    },
    #[command(name = "daemon-stop")]
    DaemonStop,
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    Keybox {
        #[command(subcommand)]
        action: KeyboxAction,
    },
    #[command(name = "security-patch")]
    SecurityPatch {
        #[command(subcommand)]
        action: SecurityPatchAction,
    },
    Conflict {
        #[command(subcommand)]
        action: ConflictAction,
    },
    Vbhash {
        #[command(subcommand)]
        action: VbhashAction,
    },
    Health {
        #[command(subcommand)]
        action: HealthAction,
    },
    Status {
        #[command(subcommand)]
        action: StatusAction,
    },
    Automation {
        #[command(subcommand)]
        action: AutomationAction,
    },
    #[command(name = "webui-init")]
    WebuiInit,
    Applist {
        #[command(subcommand)]
        action: ApplistAction,
    },
    Module {
        #[command(subcommand)]
        action: ModuleAction,
    },
    #[command(name = "daemon-status")]
    DaemonStatus,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    Get {
        key: String,
    },
    Set {
        key: String,
        value: String,
    },
    Migrate,
    List,
    Init {
        #[arg(long)]
        automation: Option<String>,
    },
    Dump {
        #[arg(long)]
        json: bool,
    },
    Defaults,
    Restore,
    #[command(name = "props-custom")]
    PropsCustom,
}

#[derive(Subcommand)]
pub enum KeyboxAction {
    Fetch,
    Validate {
        path: Option<String>,
    },
    #[command(name = "set-custom")]
    SetCustom {
        path: String,
    },
    Sources,
    Generate,
    Backup,
}

#[derive(Subcommand)]
pub enum SecurityPatchAction {
    Set,
    Update {
        #[arg(long)]
        force: bool,
    },
    Show,
    #[command(name = "set-custom")]
    SetCustom {
        system: String,
        boot: String,
        vendor: String,
    },
    #[command(name = "import-legacy")]
    ImportLegacy,
    #[command(name = "export-legacy")]
    ExportLegacy,
}

#[derive(Subcommand)]
pub enum ConflictAction {
    Check {
        #[arg(long)]
        install: bool,
    },
    Status,
}

#[derive(Subcommand)]
pub enum VbhashAction {
    Extract,
    Pass,
    Show,
}

#[derive(Subcommand)]
pub enum HealthAction {
    #[command(name = "tee-status")]
    TeeStatus,
    Status,
}

#[derive(Subcommand)]
pub enum StatusAction {
    Update,
    #[command(name = "xposed-scan")]
    XposedScan,
}

#[derive(Subcommand)]
pub enum AutomationAction {
    Status,
    Profiles,
    #[command(name = "select-profile")]
    SelectProfile {
        name: String,
    },
    Check,
    Cleanup,
    #[command(name = "sync-target")]
    SyncTarget,
    #[command(name = "export-target")]
    ExportTarget,
    #[command(name = "profile-ready")]
    ProfileReady,
}

#[derive(Subcommand)]
pub enum ApplistAction {
    List,
    Name { package: String },
    Save,
    Xposed,
}

#[derive(Subcommand)]
pub enum ModuleAction {
    #[command(name = "check-update")]
    CheckUpdate,
    #[command(name = "get-update")]
    GetUpdate,
    #[command(name = "install-update")]
    InstallUpdate,
    #[command(name = "release-note")]
    ReleaseNote,
    Uninstall,
    #[command(name = "update-locales")]
    UpdateLocales,
    Download {
        url: String,
    },
}

pub fn dispatch(command: Commands, cfg: &crate::config::Config) -> anyhow::Result<()> {
    handlers::dispatch(command, cfg)
}
