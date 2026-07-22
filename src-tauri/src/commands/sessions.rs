use std::path::PathBuf;

use serde::Deserialize;
use tauri::{AppHandle, State, ipc::Channel};

use crate::error::Result;
use crate::sessions::state::CloseReason;
use crate::sessions::{Program, SessionInfo, SessionTask};
use crate::state::AppState;

/// The IPC face of [`SessionTask`]: permission fields are not exposed —
/// UI-spawned sessions always run with default permissions; only internal
/// callers (curation, cut planning) set them.
#[derive(Debug, Deserialize, specta::Type)]
pub struct NewSession {
    pub label: String,
    pub cwd: String,
    pub initial_prompt: Option<String>,
}

/// Spawn a new claude dock session; returns its session id.
#[tauri::command]
#[specta::specta]
pub async fn spawn_session(
    app: AppHandle,
    state: State<'_, AppState>,
    task: NewSession,
) -> Result<String> {
    let task = SessionTask {
        label: task.label,
        cwd: PathBuf::from(task.cwd),
        initial_prompt: task.initial_prompt,
        append_system_prompt: None,
        permission_mode: None,
        permission_allow: vec![],
    };
    state.sessions.spawn(&app, task, Program::Claude).await
}

/// Attach the dock terminal to a session's output: scrollback replays first,
/// then live 16 ms / 16 KB batches stream over the channel.
#[tauri::command]
#[specta::specta]
pub async fn attach_session(
    state: State<'_, AppState>,
    id: String,
    on_data: Channel<Vec<u8>>,
) -> Result<()> {
    state.sessions.attach(&id, on_data)
}

/// Drop a session's output sink when its terminal unmounts (dock hidden or
/// tab switched away): the backend stops streaming into a disposed terminal
/// instead of waiting for a next attach. Idempotent; unknown ids are a no-op
/// (the session may have closed while the panel was hidden).
#[tauri::command]
#[specta::specta]
pub async fn detach_session(state: State<'_, AppState>, id: String) -> Result<()> {
    state.sessions.detach(&id);
    Ok(())
}

/// Forward xterm keystrokes (already encoded by xterm's onData) to the PTY.
#[tauri::command]
#[specta::specta]
pub async fn write_session(state: State<'_, AppState>, id: String, data: String) -> Result<()> {
    state.sessions.write(&id, data.as_bytes())
}

/// Propagate an xterm resize to the PTY.
#[tauri::command]
#[specta::specta]
pub async fn resize_session(
    state: State<'_, AppState>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<()> {
    state.sessions.resize(&id, cols, rows)
}

/// Close a session at the owner's request.
#[tauri::command]
#[specta::specta]
pub async fn close_session(state: State<'_, AppState>, id: String) -> Result<()> {
    state.sessions.close(&id, CloseReason::UserClosed).await
}

/// Every session the pool knows about, oldest-first.
#[tauri::command]
#[specta::specta]
pub async fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionInfo>> {
    Ok(state.sessions.list())
}

/// The frontend reports dock visibility/focus: drives reap exemption and
/// needs-input notification suppression.
#[tauri::command]
#[specta::specta]
pub async fn set_dock_focus(
    state: State<'_, AppState>,
    open: bool,
    focused_session: Option<String>,
) -> Result<()> {
    state.sessions.set_dock_focus(open, focused_session);
    Ok(())
}
