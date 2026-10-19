use serde::Serialize;

/// Error returned to the webview, serialized as `{ kind, detail, message }`.
///
/// The UI translates `kind` and falls back to `message`, which stays English for logs.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("game folder not found: {0}")]
    GameNotFound(String),
    #[error("game is not configured")]
    GameNotConfigured,
    #[error("SMAPI is not installed")]
    SmapiMissing,
    #[error("game is already running")]
    AlreadyRunning,
    #[error("game is not running")]
    NotRunning,
    #[error("an operation is already in progress")]
    Busy,
    #[error("network error: {0}")]
    Network(String),
    #[error("unexpected archive layout: {0}")]
    BadArchive(String),
    #[error("SMAPI installer failed: {0}")]
    InstallerFailed(String),
    #[error("invalid manifest.json: {0}")]
    BadManifest(String),
    #[error("no direct download for this mod: {0}")]
    NoDownload(String),
    #[error("not a GitHub repo: {0}")]
    BadRepo(String),
    #[error("not a mod list: {0}")]
    BadModList(String),
    #[error("no mods found in the archive")]
    NoMods,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("profile name is empty or taken: {0}")]
    BadProfileName(String),
    #[error("the last profile can't be deleted")]
    LastProfile,
    #[error("Nexus rejected the API key")]
    NexusKeyRejected,
    #[error("Nexus needs a Premium account or a Mod Manager Download link for this file")]
    NexusNeedsPremium,
    #[error("no Nexus API key is set")]
    NexusNoKey,
    #[error("not a Stardew Valley nxm link: {0}")]
    BadNxm(String),
    #[error("Keychain error: {0}")]
    Keychain(String),
    #[error("config.json can't be read: {0}")]
    BadConfig(String),
    #[error("save file can't be read: {0}")]
    BadSave(String),
    #[error("Rosetta 2 is required to run the game")]
    RosettaMissing,
    #[error("cancelled by the user")]
    Cancelled,
    #[error("close the game before changing installed mods")]
    GameRunning,
    #[error("the game isn't connected to the launcher")]
    CompanionOffline,
    #[error("no save is loaded in the game")]
    NoWorld,
    #[error("the game couldn't answer: {0}")]
    CompanionFailed(String),
    #[error("file system error: {0}")]
    Io(#[from] std::io::Error),
}

impl AppError {
    fn kind(&self) -> &'static str {
        match self {
            Self::GameNotFound(_) => "game_not_found",
            Self::GameNotConfigured => "game_not_configured",
            Self::SmapiMissing => "smapi_missing",
            Self::AlreadyRunning => "already_running",
            Self::NotRunning => "not_running",
            Self::Busy => "busy",
            Self::Network(_) => "network",
            Self::BadArchive(_) => "bad_archive",
            Self::InstallerFailed(_) => "installer_failed",
            Self::BadManifest(_) => "bad_manifest",
            Self::NoMods => "no_mods",
            Self::NoDownload(_) => "no_download",
            Self::BadRepo(_) => "bad_repo",
            Self::BadModList(_) => "bad_mod_list",
            Self::NotFound(_) => "not_found",
            Self::BadProfileName(_) => "bad_profile_name",
            Self::LastProfile => "last_profile",
            Self::NexusKeyRejected => "nexus_key_rejected",
            Self::NexusNeedsPremium => "nexus_needs_premium",
            Self::NexusNoKey => "nexus_no_key",
            Self::BadNxm(_) => "bad_nxm",
            Self::Keychain(_) => "keychain",
            Self::BadSave(_) => "bad_save",
            Self::BadConfig(_) => "bad_config",
            Self::RosettaMissing => "rosetta_missing",
            Self::Cancelled => "cancelled",
            Self::GameRunning => "game_running",
            Self::CompanionOffline => "companion_offline",
            Self::NoWorld => "no_world",
            Self::CompanionFailed(_) => "companion_failed",
            Self::Io(_) => "io",
        }
    }

    fn detail(&self) -> Option<String> {
        match self {
            Self::GameNotFound(s)
            | Self::Network(s)
            | Self::BadArchive(s)
            | Self::InstallerFailed(s)
            | Self::BadManifest(s)
            | Self::NotFound(s)
            | Self::NoDownload(s)
            | Self::BadRepo(s)
            | Self::BadModList(s)
            | Self::BadNxm(s)
            | Self::BadSave(s)
            | Self::BadConfig(s)
            | Self::Keychain(s)
            | Self::BadProfileName(s)
            | Self::CompanionFailed(s) => Some(s.clone()),
            Self::Io(e) => Some(e.to_string()),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        Self::Network(e.to_string())
    }
}

impl From<zip::result::ZipError> for AppError {
    fn from(e: zip::result::ZipError) -> Self {
        Self::BadArchive(e.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("AppError", 3)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("detail", &self.detail())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;
