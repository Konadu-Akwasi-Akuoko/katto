use katto_engine::planner::{PlanError, parse_cuts_json};
use katto_engine::schema::{Cuts, Transcript};
use katto_engine::validate::validate_cuts;

/// One poll of the bundle's `cuts.json` during a dock planning run.
#[derive(Debug)]
pub enum PlanFileVerdict {
    /// The session hasn't written the file yet — keep polling.
    Missing,
    /// Parsed and passed every invariant.
    Valid(Cuts),
    /// Present but wrong; `errors_message` is correction-turn-ready text.
    Invalid { errors_message: String },
}

/// Classify a `cuts.json` read: parse, then run the same invariant validation
/// the engine's retry loop applies, so the dock path guarantees exactly what
/// the subprocess path does.
pub fn evaluate_plan_file(
    read: std::io::Result<String>,
    transcript: &Transcript,
) -> PlanFileVerdict {
    let content = match read {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return PlanFileVerdict::Missing,
        Err(err) => {
            return PlanFileVerdict::Invalid {
                errors_message: err.to_string(),
            };
        }
        Ok(content) => content,
    };
    let cuts = match parse_cuts_json(&content) {
        Ok(cuts) => cuts,
        Err(PlanError::Invalid { error, .. }) => {
            return PlanFileVerdict::Invalid {
                errors_message: error,
            };
        }
        Err(other) => {
            return PlanFileVerdict::Invalid {
                errors_message: other.to_string(),
            };
        }
    };
    let errors = validate_cuts(&cuts, transcript);
    if errors.is_empty() {
        PlanFileVerdict::Valid(cuts)
    } else {
        PlanFileVerdict::Invalid {
            errors_message: errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; "),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error, ErrorKind};

    const TRANSCRIPT: &str =
        include_str!("../../../crates/katto-engine/tests/fixtures/transcript.valid.json");
    const CUTS: &str = include_str!("../../../crates/katto-engine/tests/fixtures/cuts.valid.json");
    const CUTS_OVERLAP: &str =
        include_str!("../../../crates/katto-engine/tests/fixtures/cuts.overlap.json");

    fn transcript() -> katto_engine::schema::Transcript {
        serde_json::from_str(TRANSCRIPT).unwrap()
    }

    #[test]
    fn missing_file_is_missing() {
        let verdict = evaluate_plan_file(Err(Error::new(ErrorKind::NotFound, "no")), &transcript());
        assert!(matches!(verdict, PlanFileVerdict::Missing));
    }

    #[test]
    fn other_io_error_is_invalid_with_message() {
        let verdict = evaluate_plan_file(
            Err(Error::new(ErrorKind::PermissionDenied, "denied")),
            &transcript(),
        );
        match verdict {
            PlanFileVerdict::Invalid { errors_message } => {
                assert!(errors_message.contains("denied"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn unparseable_json_is_invalid_with_message() {
        match evaluate_plan_file(Ok("not json".into()), &transcript()) {
            PlanFileVerdict::Invalid { errors_message } => {
                assert!(!errors_message.is_empty());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn invariant_violation_is_invalid() {
        match evaluate_plan_file(Ok(CUTS_OVERLAP.into()), &transcript()) {
            PlanFileVerdict::Invalid { errors_message } => {
                assert!(!errors_message.is_empty());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn valid_cuts_json_parses() {
        assert!(matches!(
            evaluate_plan_file(Ok(CUTS.into()), &transcript()),
            PlanFileVerdict::Valid(_)
        ));
    }
}
