//! The validate-retry-once loop shared by both planner modes, generic over an
//! attempt driver so it is testable with scripted stubs.

use crate::planner::{PlanError, parse_cuts_json};
use crate::schema::{Cuts, Transcript};
use crate::validate::validate_cuts;

/// One planner transport: a first attempt plus a correction turn.
pub(crate) trait AttemptDriver {
    /// Conversation state threaded from one attempt to the next.
    type Attempt;
    /// Run the first attempt on the serialized transcript.
    async fn first(&self, transcript_json: &str) -> Result<(String, Self::Attempt), PlanError>;
    /// Run the correction turn with the PRD correction message.
    async fn correction(
        &self,
        prior: Self::Attempt,
        message: &str,
    ) -> Result<(String, Self::Attempt), PlanError>;
}

/// Parse + validate; on invalid JSON or invariant violation retry exactly once
/// with the correction message; second failure surfaces the raw output (PRD).
/// Transport errors propagate immediately, no retry.
pub(crate) async fn plan_with_retry<D: AttemptDriver>(
    driver: &D,
    transcript: &Transcript,
) -> std::result::Result<Cuts, PlanError> {
    let transcript_json = serde_json::to_string(transcript).map_err(|e| PlanError::Invalid {
        error: format!("transcript serialization: {e}"),
        raw: String::new(),
    })?;

    let (raw, attempt) = driver.first(&transcript_json).await?;
    let message = match check(&raw, transcript) {
        Ok(cuts) => return Ok(cuts),
        Err(error) => prd_sentence(&error),
    };

    let (raw2, _) = driver.correction(attempt, &message).await?;
    check(&raw2, transcript).map_err(|error| PlanError::InvalidAfterRetry { error, raw: raw2 })
}

/// Parse then validate; the error string is what the correction turn reports.
fn check(raw: &str, transcript: &Transcript) -> std::result::Result<Cuts, String> {
    let cuts = parse_cuts_json(raw).map_err(|e| match e {
        PlanError::Invalid { error, .. } => error,
        other => other.to_string(),
    })?;
    let errors = validate_cuts(&cuts, transcript);
    if errors.is_empty() {
        return Ok(cuts);
    }
    Err(errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; "))
}

/// Wrap any error text in the PRD correction sentence (parse failures reuse
/// the same wording [`correction_message`] produces for invariant violations).
fn prd_sentence(error: &str) -> String {
    format!(
        "the JSON you returned was invalid: {error}; return only valid JSON matching the schema"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ScriptedDriver {
        replies: std::sync::Mutex<Vec<String>>,
        corrections_seen: std::sync::Mutex<Vec<String>>,
    }

    impl ScriptedDriver {
        fn new(replies: Vec<String>) -> Self {
            Self {
                replies: replies.into(),
                corrections_seen: Vec::new().into(),
            }
        }
    }

    impl AttemptDriver for ScriptedDriver {
        type Attempt = ();
        async fn first(&self, _t: &str) -> Result<(String, ()), PlanError> {
            Ok((self.replies.lock().unwrap().remove(0), ()))
        }
        async fn correction(&self, _p: (), msg: &str) -> Result<(String, ()), PlanError> {
            self.corrections_seen.lock().unwrap().push(msg.to_string());
            Ok((self.replies.lock().unwrap().remove(0), ()))
        }
    }

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    fn valid_raw() -> String {
        fixture("cuts.valid.json")
    }

    fn transcript() -> Transcript {
        serde_json::from_str(&fixture("transcript.valid.json")).unwrap()
    }

    #[tokio::test]
    async fn valid_first_attempt_needs_no_retry() {
        let d = ScriptedDriver::new(vec![valid_raw()]);
        let cuts = plan_with_retry(&d, &transcript()).await.unwrap();
        assert_eq!(cuts.cuts.len(), 2);
        assert!(d.corrections_seen.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn invalid_then_valid_retries_once_with_prd_message() {
        let d = ScriptedDriver::new(vec!["garbage".into(), valid_raw()]);
        assert!(plan_with_retry(&d, &transcript()).await.is_ok());
        let seen = d.corrections_seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert!(seen[0].starts_with("the JSON you returned was invalid: "));
        assert!(seen[0].ends_with("; return only valid JSON matching the schema"));
    }

    #[tokio::test]
    async fn invariant_violation_also_triggers_retry() {
        // bad-total fixture parses fine but fails validation
        let d = ScriptedDriver::new(vec![fixture("cuts.bad-total.json"), valid_raw()]);
        assert!(plan_with_retry(&d, &transcript()).await.is_ok());
        assert_eq!(d.corrections_seen.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn second_failure_surfaces_raw_output() {
        let d = ScriptedDriver::new(vec!["garbage".into(), "still garbage".into()]);
        match plan_with_retry(&d, &transcript()).await {
            Err(PlanError::InvalidAfterRetry { raw, .. }) => assert_eq!(raw, "still garbage"),
            other => panic!("expected InvalidAfterRetry, got {other:?}"),
        }
    }

    struct FailingDriver;
    impl AttemptDriver for FailingDriver {
        type Attempt = ();
        async fn first(&self, _t: &str) -> Result<(String, ()), PlanError> {
            Err(PlanError::Auth("bad key".into()))
        }
        async fn correction(&self, _p: (), _m: &str) -> Result<(String, ()), PlanError> {
            panic!("transport errors must not retry");
        }
    }

    #[tokio::test]
    async fn transport_errors_propagate_without_retry() {
        assert!(matches!(
            plan_with_retry(&FailingDriver, &transcript()).await,
            Err(PlanError::Auth(_))
        ));
    }
}
