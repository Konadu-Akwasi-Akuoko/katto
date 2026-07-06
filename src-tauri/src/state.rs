use crate::db::DbHandle;

/// Application-wide managed state, injected into every command via
/// `State<'_, AppState>`.
pub struct AppState {
    pub db: DbHandle,
}
