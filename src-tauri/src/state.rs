use crate::db::DbHandle;
use crate::jobs::JobRuntime;

/// Application-wide managed state, injected into every command via
/// `State<'_, AppState>`.
pub struct AppState {
    pub db: DbHandle,
    pub jobs: JobRuntime,
}
