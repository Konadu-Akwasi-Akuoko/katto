//! Cut-planner foundation: the committed cut-decider prompt, the single-shot
//! [`CutPlanner`] trait (D14: no agent loop), and tolerant reply parsers.

pub mod http;
pub mod partial;
pub(crate) mod retry;
pub mod subprocess;

use crate::schema::Cuts;
use crate::validate::ValidationError;

/// The committed cut-decider system prompt (external input, body verbatim).
pub const CUT_DECIDER_PROMPT: &str = include_str!("../prompts/cut-decider.md");

/// Appended to the system prompt in both planner modes: overrides the file's
/// Write-tool output discipline for a toolless, single-shot reply.
pub const OUTPUT_OVERRIDE: &str = "\n\n## Runtime override (katto)\n\nYou are running as a single-shot planner with NO tools. Do not attempt to read or write any file. Reply with ONLY the cuts.json JSON object as plain text - no prose, no markdown fences, no commentary.\n";

/// Planner failures, split by transport vs invalid-output (only the latter retries).
#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    /// The claude subprocess failed (spawn, non-zero exit, or is_error envelope).
    #[error("claude subprocess: {0}")]
    Subprocess(String),
    /// The Anthropic HTTP API failed (transport or non-auth error response).
    #[error("anthropic api: {0}")]
    Http(String),
    /// The Anthropic API rejected the key (401/403).
    #[error("anthropic auth: {0}")]
    Auth(String),
    /// First attempt returned unparseable/invalid output (retried once).
    #[error("planner returned invalid output: {error}")]
    Invalid {
        /// What was wrong with the output.
        error: String,
        /// The raw planner reply.
        raw: String,
    },
    /// Second failure: surfaced with the raw output as a debugging aid (PRD).
    #[error("planner output invalid after retry: {error}\n--- raw planner output ---\n{raw}")]
    InvalidAfterRetry {
        /// What was wrong with the output.
        error: String,
        /// The raw planner reply.
        raw: String,
    },
}

/// Single-shot cut planner (D14: no agent loop).
pub trait CutPlanner {
    /// Plan cuts for `transcript`; implementations run the validate-retry-once loop.
    fn plan(
        &self,
        transcript: &crate::schema::Transcript,
    ) -> impl std::future::Future<Output = std::result::Result<Cuts, PlanError>> + Send;
}

/// Parse planner reply text into [`Cuts`], tolerating markdown fences and
/// leading prose by extracting the first balanced top-level JSON object.
///
/// # Errors
/// [`PlanError::Invalid`] carrying the raw reply when no valid object is found.
pub fn parse_cuts_json(raw: &str) -> std::result::Result<Cuts, PlanError> {
    let invalid = |error: String| PlanError::Invalid {
        error,
        raw: raw.to_owned(),
    };
    let start = raw
        .find('{')
        .ok_or_else(|| invalid("no JSON object in planner output".into()))?;
    let slice = balanced_object(&raw[start..])
        .ok_or_else(|| invalid("unterminated JSON object in planner output".into()))?;
    serde_json::from_str(slice).map_err(|e| invalid(e.to_string()))
}

/// Render validation errors into the retry correction message (PRD wording).
pub fn correction_message(errors: &[ValidationError]) -> String {
    let lines = errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ");
    format!(
        "the JSON you returned was invalid: {lines}; return only valid JSON matching the schema"
    )
}

/// Runtime planner selection (claude detected -> Subprocess, else BYOK Http).
#[derive(Debug)]
pub enum Planner {
    /// Subscription-auth claude subprocess.
    Subprocess(subprocess::SubprocessClaudePlanner),
    /// BYOK Anthropic Messages API.
    Http(http::HttpAnthropicPlanner),
}

impl CutPlanner for Planner {
    async fn plan(
        &self,
        transcript: &crate::schema::Transcript,
    ) -> std::result::Result<Cuts, PlanError> {
        match self {
            Planner::Subprocess(p) => p.plan(transcript).await,
            Planner::Http(p) => p.plan(transcript).await,
        }
    }
}

/// Return the slice of `s` covering the first balanced `{...}` object, or
/// `None` if it never closes. `s` must start at a `{`.
pub(crate) fn balanced_object(s: &str) -> Option<&str> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(&s[..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn prompt_is_embedded_and_frontmatter_free() {
        assert!(CUT_DECIDER_PROMPT.starts_with("# cut-decider"));
        assert!(CUT_DECIDER_PROMPT.contains("Cut Policy"));
        assert!(!CUT_DECIDER_PROMPT.contains("model:"));
    }

    #[test]
    fn parse_accepts_bare_json() {
        assert!(parse_cuts_json(&fixture("cuts.valid.json")).is_ok());
    }

    #[test]
    fn parse_strips_fences_and_prose() {
        let raw = format!(
            "Here is the plan:\n```json\n{}\n```\nDone.",
            fixture("cuts.valid.json")
        );
        assert!(parse_cuts_json(&raw).is_ok());
    }

    #[test]
    fn parse_failure_carries_raw() {
        match parse_cuts_json("not json at all") {
            Err(PlanError::Invalid { raw, .. }) => assert_eq!(raw, "not json at all"),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn correction_message_uses_prd_wording() {
        let errs = vec![ValidationError::TotalMismatch {
            stated: 1.0,
            computed: 2.0,
        }];
        let msg = correction_message(&errs);
        assert!(msg.starts_with("the JSON you returned was invalid: "));
        assert!(msg.ends_with("; return only valid JSON matching the schema"));
    }
}
