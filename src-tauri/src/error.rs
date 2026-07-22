use serde::Serialize;

/// The application error type crossing every command boundary.
///
/// Each variant carries a pre-formatted message string, and serde derives the
/// tagged `{ kind, message }` wire shape (`kind` = snake_case variant name). The
/// frontend switches on the stable `kind` slug. `specta::Type` (added alongside
/// the command layer) types the same shape for the generated bindings, keeping
/// Rust and TypeScript in lockstep without a hand-written impl.
#[derive(Debug, thiserror::Error, Serialize, specta::Type)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
pub enum Error {
    #[error("{0}")]
    Db(String),

    #[error("{0}")]
    Migration(String),

    #[error("{0}")]
    JobTransition(String),

    #[error("{0}")]
    Io(String),

    #[error("{0}")]
    DbClosed(String),

    #[error("{0}")]
    Keychain(String),

    #[error("{0}")]
    Onboarding(String),

    #[error("{0}")]
    Autostart(String),

    #[error("{0}")]
    StudioRootUnmounted(String),

    #[error("{0}")]
    InvalidManifest(String),

    #[error("{0}")]
    PromoteFailed(String),

    #[error("{0}")]
    ShortcutInvalid(String),

    #[error("{0}")]
    ShortcutUnavailable(String),

    #[error("{0}")]
    Engine(String),

    #[error("{0}")]
    NoSuchProject(String),

    #[error("{0}")]
    InsufficientSpace(String),

    #[error("{0}")]
    EjectFailed(String),

    #[error("{0}")]
    IngestInvalid(String),

    #[error("{0}")]
    MissingKey(String),

    #[error("{0}")]
    NoPlanner(String),

    #[error("{0}")]
    PipelineBusy(String),

    #[error("{0}")]
    Relocate(String),

    #[error("{0}")]
    ClaudeMissing(String),

    #[error("{0}")]
    SessionNotFound(String),

    #[error("{0}")]
    SessionSpawn(String),

    #[error("{0}")]
    InvalidName(String),

    #[error("{0}")]
    NoSuchScheduledJob(String),

    #[error("{0}")]
    InvalidSchedule(String),

    #[error("unzip failed: {0}")]
    UnzipFailed(String),

    #[error("browser unavailable: {0}")]
    BrowserUnavailable(String),

    #[error("no parked download with id {0}")]
    DownloadMissing(String),

    #[error("{0}")]
    ResolveNotInstalled(String),

    #[error("{0}")]
    ResolveNotRunning(String),

    #[error("{0}")]
    ResolveFailed(String),

    #[error("{0}")]
    ImportFailed(String),

    /// The one structured variant: the relocation surface needs the fields
    /// (name a file, show its duration), not a flattened string. On the wire
    /// `message` becomes an object for this kind only; the IPC wrapper
    /// re-derives a display string from it.
    #[error("source missing: expected {expected_path}")]
    SourceMissing {
        expected_path: String,
        filename: String,
        /// Display projection (`to_secs_f64`) of the manifest duration.
        duration_secs: f64,
    },
}

impl Error {
    /// The single dedicated DB writer thread is gone (its channel closed). Carries
    /// a message so every variant keeps the uniform `{ kind, message }` wire shape.
    pub fn db_closed() -> Self {
        Error::DbClosed("the database writer is unavailable".to_string())
    }

    /// A `project.json` failed schema/slug validation. Pre-formats the offending
    /// path into the message so the variant stays a single-string tuple and keeps
    /// the uniform `{ kind, message: string }` wire shape the frontend switches on.
    pub fn invalid_manifest(path: &std::path::Path, message: &str) -> Self {
        Error::InvalidManifest(format!("{}: {message}", path.display()))
    }

    /// Promoting an idea into a project failed. Pre-formats the failing `stage`
    /// into the message so the variant stays a single-string tuple and keeps the
    /// uniform `{ kind, message: string }` wire shape the frontend switches on
    /// (a struct variant would nest an object under `message` and break that).
    pub fn promote_failed(stage: &str, message: &str) -> Self {
        Error::PromoteFailed(format!("promote failed at {stage}: {message}"))
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Error::Db(err.to_string())
    }
}

impl From<rusqlite_migration::Error> for Error {
    fn from(err: rusqlite_migration::Error) -> Self {
        Error::Migration(err.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err.to_string())
    }
}

impl From<keyring_core::Error> for Error {
    fn from(err: keyring_core::Error) -> Self {
        Error::Keychain(err.to_string())
    }
}

impl From<katto_engine::Error> for Error {
    fn from(err: katto_engine::Error) -> Self {
        match err {
            // Its own wire kind: the editor renders relocation copy for it.
            katto_engine::Error::SourceMissing {
                expected_path,
                filename,
                duration,
            } => Error::SourceMissing {
                expected_path: expected_path.to_string_lossy().into_owned(),
                filename,
                duration_secs: duration.to_secs_f64(),
            },
            // Keeps its wire kind: the relocate dialog renders it inline.
            katto_engine::Error::Relocate(msg) => Error::Relocate(msg),
            other => Error::Engine(other.to_string()),
        }
    }
}

impl From<tauri_plugin_autostart::Error> for Error {
    fn from(err: tauri_plugin_autostart::Error) -> Self {
        Error::Autostart(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
