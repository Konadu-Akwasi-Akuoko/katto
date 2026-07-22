//! Subscription-auth claude subprocess planner (D14). The argv builders and
//! NDJSON stream accumulator are pure and unit-tested; the spawn sites stay
//! thin. Never passes `--bare` (would skip OAuth/keychain auth) and never
//! passes `--model` (subscription default).

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::planner::partial::cuts_prefix;
use crate::planner::{CUT_DECIDER_PROMPT, OUTPUT_OVERRIDE, PlanError};
use crate::schema::Cut;

/// Streaming observer for incremental cut arrival (app wires this to Channel events).
pub trait PartialObserver: Send + Sync {
    /// Called whenever the count of parseable cuts in the accumulating reply grows.
    fn on_cuts(&self, cuts: &[Cut]);
}

/// Cut planner backed by the local `claude` CLI on subscription auth.
pub struct SubprocessClaudePlanner {
    /// Path to the claude binary (from detection, not hardcoded).
    pub claude_path: PathBuf,
    /// Working directory for both calls (`--resume` is directory-scoped).
    pub workdir: PathBuf,
    /// Optional incremental-cuts observer.
    pub observer: Option<Arc<dyn PartialObserver>>,
    /// Whole-call timeout (default 10 minutes).
    pub timeout: Duration,
}

impl std::fmt::Debug for SubprocessClaudePlanner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubprocessClaudePlanner")
            .field("claude_path", &self.claude_path)
            .field("workdir", &self.workdir)
            .field("observer", &self.observer.is_some())
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl SubprocessClaudePlanner {
    /// Build a planner with the default 10-minute timeout and no observer.
    pub fn new(claude_path: PathBuf, workdir: PathBuf) -> Self {
        Self {
            claude_path,
            workdir,
            observer: None,
            timeout: Duration::from_secs(600),
        }
    }

    /// Attach an incremental-cuts observer.
    #[must_use]
    pub fn with_observer(mut self, obs: Arc<dyn PartialObserver>) -> Self {
        self.observer = Some(obs);
        self
    }
}

/// Argv for the first streaming attempt.
pub fn first_attempt_argv() -> Vec<String> {
    vec![
        "-p".into(),
        "--output-format".into(),
        "stream-json".into(),
        "--verbose".into(),
        "--include-partial-messages".into(),
        "--system-prompt".into(),
        format!("{CUT_DECIDER_PROMPT}{OUTPUT_OVERRIDE}"),
    ]
}

/// Argv for the session-resumed correction attempt.
pub fn correction_argv(session_id: &str) -> Vec<String> {
    vec![
        "-p".into(),
        "--output-format".into(),
        "json".into(),
        "--resume".into(),
        session_id.into(),
    ]
}

/// The final `result` line of a stream-json run.
#[derive(Debug, Clone, PartialEq)]
pub struct FinalEnvelope {
    /// The full reply text (authoritative over the delta accumulation).
    pub text: String,
    /// The session id for `--resume`.
    pub session_id: Option<String>,
    /// Whether the CLI reported the run as an error.
    pub is_error: bool,
}

/// Accumulates NDJSON stream lines into reply text plus the final envelope. Pure.
#[derive(Debug, Default)]
pub struct StreamAccum {
    text: String,
    final_: Option<FinalEnvelope>,
}

impl StreamAccum {
    /// Feed one NDJSON line; returns true when visible reply text grew.
    /// Unknown line shapes are ignored (forward-compatible).
    pub fn push_line(&mut self, line: &str) -> bool {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            return false;
        };
        match v.get("type").and_then(Value::as_str) {
            Some("stream_event") => {
                if !v.get("parent_tool_use_id").is_none_or(Value::is_null) {
                    return false; // subagent output, not the main conversation
                }
                let delta = v.get("event").and_then(|e| e.get("delta"));
                let is_text =
                    delta.and_then(|d| d.get("type")).and_then(Value::as_str) == Some("text_delta");
                if !is_text {
                    return false;
                }
                if let Some(text) = delta.and_then(|d| d.get("text")).and_then(Value::as_str)
                    && !text.is_empty()
                {
                    self.text.push_str(text);
                    return true;
                }
                false
            }
            Some("result") => {
                self.final_ = Some(FinalEnvelope {
                    text: v
                        .get("result")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    session_id: v
                        .get("session_id")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    is_error: v.get("is_error").and_then(Value::as_bool).unwrap_or(false),
                });
                false
            }
            _ => false,
        }
    }

    /// The reply text accumulated so far.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The final envelope, if the `result` line arrived.
    pub fn finish(self) -> Option<FinalEnvelope> {
        self.final_
    }
}

/// One raw planner reply plus the session id for a possible correction resume.
pub(crate) struct RawAttempt {
    pub text: String,
    pub session_id: Option<String>,
}

impl SubprocessClaudePlanner {
    /// First attempt: streaming call with the transcript on stdin.
    pub(crate) async fn first(&self, transcript_json: &str) -> Result<RawAttempt, PlanError> {
        self.run_streaming(&first_attempt_argv(), transcript_json)
            .await
    }

    /// Correction attempt: resume the session when we have its id, else a
    /// fresh first-style call with transcript + correction on stdin.
    pub(crate) async fn correction(
        &self,
        session_id: Option<&str>,
        transcript_json: &str,
        message: &str,
    ) -> Result<RawAttempt, PlanError> {
        match session_id {
            Some(id) => self.run_json(&correction_argv(id), message).await,
            None => {
                let stdin = format!("{transcript_json}\n\n{message}");
                self.run_streaming(&first_attempt_argv(), &stdin).await
            }
        }
    }

    /// Thin spawn site for the streaming first attempt.
    async fn run_streaming(
        &self,
        argv: &[String],
        stdin_text: &str,
    ) -> Result<RawAttempt, PlanError> {
        let sub = |e: String| PlanError::Subprocess(e);
        let run = async {
            // kill_on_drop: a timeout cancels this future mid-await; the child
            // must die with it, not keep burning the subscription session.
            let mut child = tokio::process::Command::new(&self.claude_path)
                .current_dir(&self.workdir)
                .args(argv)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .map_err(|e| sub(format!("spawn: {e}")))?;

            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(stdin_text.as_bytes())
                    .await
                    .map_err(|e| sub(format!("stdin: {e}")))?;
                drop(stdin);
            }

            // Drain stderr concurrently with the stdout line loop — a child
            // filling the stderr pipe while we only read stdout would deadlock.
            let stderr_pipe = child.stderr.take();
            let stderr_task = tokio::spawn(async move {
                let mut buf = Vec::new();
                if let Some(mut pipe) = stderr_pipe {
                    let _ = tokio::io::AsyncReadExt::read_to_end(&mut pipe, &mut buf).await;
                }
                buf
            });

            let stdout = child.stdout.take().ok_or_else(|| sub("no stdout".into()))?;
            let mut lines = BufReader::new(stdout).lines();
            let mut accum = StreamAccum::default();
            let mut seen_cuts = 0usize;
            while let Some(line) = lines
                .next_line()
                .await
                .map_err(|e| sub(format!("stdout: {e}")))?
            {
                if accum.push_line(&line)
                    && let Some(obs) = &self.observer
                {
                    let cuts = cuts_prefix(accum.text());
                    if cuts.len() > seen_cuts {
                        seen_cuts = cuts.len();
                        obs.on_cuts(&cuts);
                    }
                }
            }

            let status = child.wait().await.map_err(|e| sub(format!("wait: {e}")))?;
            let stderr_bytes = stderr_task.await.unwrap_or_default();
            let stderr = String::from_utf8_lossy(&stderr_bytes);
            let envelope = accum.finish();
            let failed = !status.success() || envelope.as_ref().is_none_or(|e| e.is_error);
            if failed {
                let text = envelope.as_ref().map(|e| e.text.as_str()).unwrap_or("");
                return Err(sub(format!(
                    "claude exited with {status}: {} {}",
                    stderr.trim(),
                    text
                )));
            }
            let envelope = envelope.ok_or_else(|| sub("no result line".into()))?;
            Ok(RawAttempt {
                text: envelope.text,
                session_id: envelope.session_id,
            })
        };
        tokio::time::timeout(self.timeout, run)
            .await
            .map_err(|_| sub(format!("timed out after {:?}", self.timeout)))?
    }

    /// Thin spawn site for the single-object `--output-format json` correction.
    async fn run_json(&self, argv: &[String], stdin_text: &str) -> Result<RawAttempt, PlanError> {
        let sub = |e: String| PlanError::Subprocess(e);
        let run = async {
            let mut child = tokio::process::Command::new(&self.claude_path)
                .current_dir(&self.workdir)
                .args(argv)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .map_err(|e| sub(format!("spawn: {e}")))?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(stdin_text.as_bytes())
                    .await
                    .map_err(|e| sub(format!("stdin: {e}")))?;
                drop(stdin);
            }
            let output = child
                .wait_with_output()
                .await
                .map_err(|e| sub(format!("wait: {e}")))?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let envelope: Value = serde_json::from_str(stdout.trim())
                .map_err(|e| sub(format!("claude envelope: {e}: {stderr}")))?;
            let is_error = envelope
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if !output.status.success() || is_error {
                return Err(sub(format!(
                    "claude exited with {}: {} {}",
                    output.status,
                    stderr.trim(),
                    envelope.get("result").and_then(Value::as_str).unwrap_or("")
                )));
            }
            Ok(RawAttempt {
                text: envelope
                    .get("result")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                session_id: envelope
                    .get("session_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            })
        };
        tokio::time::timeout(self.timeout, run)
            .await
            .map_err(|_| sub(format!("timed out after {:?}", self.timeout)))?
    }
}

impl crate::planner::retry::AttemptDriver for SubprocessClaudePlanner {
    // (session id for --resume, transcript json for the resume-less fallback)
    type Attempt = (Option<String>, String);

    async fn first(&self, transcript_json: &str) -> Result<(String, Self::Attempt), PlanError> {
        let raw = SubprocessClaudePlanner::first(self, transcript_json).await?;
        Ok((raw.text, (raw.session_id, transcript_json.to_owned())))
    }

    async fn correction(
        &self,
        prior: Self::Attempt,
        message: &str,
    ) -> Result<(String, Self::Attempt), PlanError> {
        let (session_id, transcript_json) = prior;
        let raw = SubprocessClaudePlanner::correction(
            self,
            session_id.as_deref(),
            &transcript_json,
            message,
        )
        .await?;
        Ok((raw.text, (raw.session_id, transcript_json)))
    }
}

impl crate::planner::CutPlanner for SubprocessClaudePlanner {
    async fn plan(
        &self,
        transcript: &crate::schema::Transcript,
    ) -> Result<crate::schema::Cuts, PlanError> {
        crate::planner::retry::plan_with_retry(self, transcript).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_builders_are_exact() {
        let argv = first_attempt_argv();
        assert_eq!(argv[0], "-p");
        assert!(argv.contains(&"stream-json".to_string()));
        assert!(argv.contains(&"--include-partial-messages".to_string()));
        let sys = &argv[argv.iter().position(|a| a == "--system-prompt").unwrap() + 1];
        assert!(sys.starts_with("# cut-decider"));
        assert!(sys.ends_with(OUTPUT_OVERRIDE));
        assert!(!argv.contains(&"--bare".to_string())); // auth: never bare (D14)
        assert!(!argv.contains(&"--model".to_string())); // subscription default

        assert_eq!(
            correction_argv("sess-1"),
            vec!["-p", "--output-format", "json", "--resume", "sess-1"]
        );
    }

    #[test]
    fn stream_accum_replays_fixture() {
        let fixture = std::fs::read_to_string(format!(
            "{}/tests/fixtures/claude.stream-json.txt",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let mut acc = StreamAccum::default();
        let mut growth_counts = Vec::new();
        for line in fixture.lines() {
            if acc.push_line(line) {
                growth_counts.push(cuts_prefix(acc.text()).len());
            }
        }
        // first delta yields 1 complete cut, second completes the 2nd
        assert!(growth_counts.contains(&1));
        assert!(growth_counts.contains(&2));
        let fin = acc.finish().unwrap();
        assert_eq!(fin.session_id.as_deref(), Some("sess-1"));
        assert!(!fin.is_error);
        assert!(crate::planner::parse_cuts_json(&fin.text).is_ok());
    }

    #[test]
    fn subagent_deltas_are_ignored() {
        let mut acc = StreamAccum::default();
        let grew = acc.push_line(
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"nope"}},"parent_tool_use_id":"tool-1"}"#,
        );
        assert!(!grew);
        assert!(acc.text().is_empty());
    }
}
