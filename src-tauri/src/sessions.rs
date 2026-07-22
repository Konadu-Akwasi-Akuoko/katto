//! Claude dock sessions (Phase 6): PTY-backed interactive `claude` runs
//! surfaced as tabs in the dock panel. Pure logic (state machine, scrollback,
//! launch assembly) lives in submodules; process/PTY spawn sites stay thin.

pub mod buffer;
pub mod state;
