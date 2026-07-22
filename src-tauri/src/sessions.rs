//! Claude dock sessions (Phase 6): PTY-backed interactive `claude` runs
//! surfaced as tabs in the dock panel. Pure logic (state machine, scrollback,
//! launch assembly) lives in submodules; process/PTY spawn sites stay thin.

pub mod buffer;
pub mod hooks_endpoint;
pub mod launch;
pub mod planfile;
pub mod pool;
pub mod pty;
pub mod reap;
pub mod state;

use std::path::PathBuf;

use serde::Serialize;

use crate::sessions::state::SessionState;

/// What a new dock session should run. Internal callers (curation, cut
/// planning) set the permission fields; the public spawn command fills
/// defaults instead of exposing them over IPC.
#[derive(Debug, Clone)]
pub struct SessionTask {
    pub label: String,
    pub cwd: PathBuf,
    pub initial_prompt: Option<String>,
    pub append_system_prompt: Option<String>,
    pub permission_mode: Option<String>,
    pub permission_allow: Vec<String>,
}

/// Which executable backs a session. `Custom` exists for tests and smoke runs
/// (plain shells like `bash -c 'cat'`); real sessions are `Claude`.
#[derive(Debug, Clone)]
pub enum Program {
    Claude,
    Custom(String),
}

/// One session as the frontend sees it (tab strip + dock icon states).
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct SessionInfo {
    pub id: String,
    pub label: String,
    pub state: SessionState,
    pub cwd: String,
    pub started_at: String,
    pub idle_since_secs: Option<u32>,
}
