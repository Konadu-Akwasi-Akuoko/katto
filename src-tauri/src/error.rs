use serde::{Serialize, Serializer, ser::SerializeStruct};

/// The application error type crossing every command boundary.
///
/// Serializes as a tagged `{ kind, message }` object so the frontend can switch
/// on a stable `kind` slug without parsing human-facing text.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("migration error: {0}")]
    Migration(#[from] rusqlite_migration::Error),

    #[error("invalid job transition: {from} -> {to}")]
    JobTransition { from: String, to: String },
}

impl Error {
    /// Stable machine-readable discriminant paired with `message` on the wire.
    fn kind(&self) -> &'static str {
        match self {
            Error::Db(_) => "db",
            Error::Migration(_) => "migration",
            Error::JobTransition { .. } => "job_transition",
        }
    }
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("Error", 2)?;
        state.serialize_field("kind", self.kind())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

pub type Result<T> = std::result::Result<T, Error>;
