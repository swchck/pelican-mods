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
    #[error("Rosetta 2 is required to run the game")]
    RosettaMissing,
    #[error("cancelled by the user")]
    Cancelled,
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
            Self::RosettaMissing => "rosetta_missing",
            Self::Cancelled => "cancelled",
            Self::Io(_) => "io",
        }
    }

    fn detail(&self) -> Option<String> {
        match self {
            Self::GameNotFound(s)
            | Self::Network(s)
            | Self::BadArchive(s)
            | Self::InstallerFailed(s) => Some(s.clone()),
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
