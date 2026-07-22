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
        Error::Engine(err.to_string())
    }
}

impl From<tauri_plugin_autostart::Error> for Error {
    fn from(err: tauri_plugin_autostart::Error) -> Self {
        Error::Autostart(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
