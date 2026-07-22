use serde::Serialize;

/// Bytes below/at this length are treated as TUI repaints in degraded mode;
/// only a larger burst counts as Claude actually resuming work.
const DEGRADED_BURST_BYTES: usize = 512;

/// Observable session lifecycle. `Failed`/`Closed` are terminal — a session is
/// never resurrected; its tab and scrollback stay visible (D18).
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionState {
    Running,
    NeedsInput,
    Idle,
    Failed { error: String },
    Closed { reason: CloseReason },
}

/// Why a session ended. `IdleReaped` drives the "closed after idle" tab note.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum CloseReason {
    Exited,
    IdleReaped,
    UserClosed,
}

/// Inputs to the state machine. Hook events come from the localhost hooks
/// endpoint; the rest are observed by the pool's PTY threads.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionEvent {
    HookStop,
    HookNotification,
    UserInput,
    OutputBytes { len: usize },
    PtyExited { code: Option<u32> },
    SilenceTimeout,
}

/// Pure transition function. `hooks_live` selects between hook-driven
/// transitions and the output-silence heuristics of degraded mode; hook events
/// are honored either way (a late-arriving hook proves the endpoint works).
pub fn apply(state: &SessionState, event: &SessionEvent, hooks_live: bool) -> SessionState {
    use SessionEvent::*;
    use SessionState::*;

    match state {
        Failed { .. } | Closed { .. } => state.clone(),
        _ => match event {
            PtyExited { code: Some(0) } => Closed {
                reason: CloseReason::Exited,
            },
            PtyExited { code } => Failed {
                error: match code {
                    Some(code) => format!("exited with status {code}"),
                    None => "exited with status unknown".to_string(),
                },
            },
            HookStop => Idle,
            HookNotification => NeedsInput,
            UserInput => Running,
            OutputBytes { len } => {
                if !hooks_live && matches!(state, Idle) && *len > DEGRADED_BURST_BYTES {
                    Running
                } else {
                    state.clone()
                }
            }
            SilenceTimeout => {
                if !hooks_live && matches!(state, Running) {
                    Idle
                } else {
                    state.clone()
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    use CloseReason::*;
    use SessionEvent::*;
    use SessionState::*;

    #[rstest]
    #[case::stop_idles(Running, HookStop, true, Idle)]
    #[case::notification_needs_input(Running, HookNotification, true, NeedsInput)]
    #[case::stop_clears_needs_input(NeedsInput, HookStop, true, Idle)]
    #[case::input_resumes_from_idle(Idle, UserInput, true, Running)]
    #[case::input_resumes_from_needs_input(NeedsInput, UserInput, true, Running)]
    #[case::output_never_changes_state_when_hooks_live(
        Idle,
        OutputBytes { len: 9000 },
        true,
        Idle
    )]
    #[case::silence_ignored_when_hooks_live(Running, SilenceTimeout, true, Running)]
    #[case::degraded_silence_idles(Running, SilenceTimeout, false, Idle)]
    #[case::degraded_burst_resumes(Idle, OutputBytes { len: 513 }, false, Running)]
    #[case::degraded_repaint_stays_idle(Idle, OutputBytes { len: 512 }, false, Idle)]
    #[case::clean_exit_closes(
        Running,
        PtyExited { code: Some(0) },
        true,
        Closed { reason: Exited }
    )]
    #[case::dirty_exit_fails(
        Idle,
        PtyExited { code: Some(1) },
        true,
        Failed { error: "exited with status 1".into() }
    )]
    #[case::signal_death_fails(
        Running,
        PtyExited { code: None },
        true,
        Failed { error: "exited with status unknown".into() }
    )]
    #[case::failed_is_terminal(
        Failed { error: "x".into() },
        HookStop,
        true,
        Failed { error: "x".into() }
    )]
    #[case::closed_is_terminal(
        Closed { reason: UserClosed },
        UserInput,
        true,
        Closed { reason: UserClosed }
    )]
    fn transition_table_holds(
        #[case] from: SessionState,
        #[case] event: SessionEvent,
        #[case] hooks_live: bool,
        #[case] want: SessionState,
    ) {
        assert_eq!(apply(&from, &event, hooks_live), want);
    }
}
