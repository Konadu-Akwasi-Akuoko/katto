use std::time::Duration;

use crate::sessions::state::SessionState;

/// The reap decision: only `Idle` sessions past the timeout go, and a session
/// focused in an open dock panel is exempt. `NeedsInput` is never reaped — the
/// owner still has to answer it.
pub fn should_reap(
    state: &SessionState,
    idle_for: Option<Duration>,
    timeout: Duration,
    exempt: bool,
) -> bool {
    matches!(state, SessionState::Idle) && !exempt && idle_for.is_some_and(|idle| idle > timeout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::state::{CloseReason, SessionState};
    use std::time::Duration;

    const FIVE_MIN: Duration = Duration::from_secs(300);

    #[test]
    fn idle_past_timeout_reaps() {
        assert!(should_reap(
            &SessionState::Idle,
            Some(Duration::from_secs(301)),
            FIVE_MIN,
            false
        ));
    }

    #[test]
    fn idle_under_timeout_survives() {
        assert!(!should_reap(
            &SessionState::Idle,
            Some(Duration::from_secs(299)),
            FIVE_MIN,
            false
        ));
    }

    #[test]
    fn focused_panel_exempts() {
        assert!(!should_reap(
            &SessionState::Idle,
            Some(Duration::from_secs(9999)),
            FIVE_MIN,
            true
        ));
    }

    #[test]
    fn needs_input_never_reaped() {
        assert!(!should_reap(
            &SessionState::NeedsInput,
            Some(Duration::from_secs(9999)),
            FIVE_MIN,
            false
        ));
    }

    #[test]
    fn running_failed_closed_never_reaped() {
        for s in [
            SessionState::Running,
            SessionState::Failed { error: "x".into() },
            SessionState::Closed {
                reason: CloseReason::Exited,
            },
        ] {
            assert!(!should_reap(
                &s,
                Some(Duration::from_secs(9999)),
                FIVE_MIN,
                false
            ));
        }
    }
}
