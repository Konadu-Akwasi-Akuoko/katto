//! BYOK Anthropic Messages API planner — the fallback when no claude binary
//! is detected. Non-streaming: cuts arrive in one batch when the response
//! lands (incremental arrival is a subprocess-mode feature).

use serde_json::{Value, json};

use crate::planner::{CUT_DECIDER_PROMPT, OUTPUT_OVERRIDE, PlanError};

/// Production Anthropic API origin.
pub const ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com";
/// PRD-locked default planner model, settings-overridable.
pub const DEFAULT_MODEL: &str = "claude-sonnet-4-6";
/// PRD-locked completion budget.
pub const MAX_TOKENS: u32 = 8192;

/// Cut planner backed by the Anthropic Messages API with the owner's key.
pub struct HttpAnthropicPlanner {
    /// The Anthropic API key (never logged; Debug redacts).
    pub api_key: String,
    /// Model id (settings `planner_model`, default [`DEFAULT_MODEL`]).
    pub model: String,
    /// API origin; injectable for tests.
    pub base_url: String,
}

impl std::fmt::Debug for HttpAnthropicPlanner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpAnthropicPlanner")
            .field("api_key", &"<redacted>")
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .finish()
    }
}

/// Pure Messages API request-body builder.
pub fn request_body(model: &str, system: &str, messages: &[(String, String)]) -> Value {
    json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "system": system,
        "messages": messages
            .iter()
            .map(|(role, content)| json!({"role": role, "content": content}))
            .collect::<Vec<_>>(),
    })
}

/// One HTTP attempt: the reply text plus the conversation so far (for the
/// correction turn).
pub(crate) struct HttpAttempt {
    pub text: String,
    pub messages: Vec<(String, String)>,
}

impl HttpAnthropicPlanner {
    /// First attempt: single user turn carrying the transcript JSON.
    pub(crate) async fn first(&self, transcript_json: &str) -> Result<HttpAttempt, PlanError> {
        let messages = vec![("user".to_string(), transcript_json.to_string())];
        let text = self.post(&messages).await?;
        Ok(HttpAttempt { text, messages })
    }

    /// Correction attempt: prior conversation + assistant reply + correction.
    pub(crate) async fn correction(
        &self,
        prior: &HttpAttempt,
        message: &str,
    ) -> Result<HttpAttempt, PlanError> {
        let mut messages = prior.messages.clone();
        messages.push(("assistant".to_string(), prior.text.clone()));
        messages.push(("user".to_string(), message.to_string()));
        let text = self.post(&messages).await?;
        Ok(HttpAttempt { text, messages })
    }

    /// Thin transport site: POST /v1/messages and extract `content[0].text`.
    async fn post(&self, messages: &[(String, String)]) -> Result<String, PlanError> {
        let system = format!("{CUT_DECIDER_PROMPT}{OUTPUT_OVERRIDE}");
        let body = request_body(&self.model, &system, messages);
        let response = reqwest::Client::new()
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| PlanError::Http(e.to_string()))?;

        let status = response.status();
        let value: Value = response
            .json()
            .await
            .map_err(|e| PlanError::Http(e.to_string()))?;

        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(PlanError::Auth(api_error_message(&value)));
        }
        if !status.is_success() {
            return Err(PlanError::Http(format!(
                "{status}: {}",
                api_error_message(&value)
            )));
        }
        value
            .get("content")
            .and_then(|c| c.get(0))
            .and_then(|b| b.get("text"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| PlanError::Http("empty content".into()))
    }
}

fn api_error_message(value: &Value) -> String {
    value
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}

impl crate::planner::retry::AttemptDriver for HttpAnthropicPlanner {
    type Attempt = HttpAttempt;

    async fn first(&self, transcript_json: &str) -> Result<(String, Self::Attempt), PlanError> {
        let attempt = HttpAnthropicPlanner::first(self, transcript_json).await?;
        Ok((attempt.text.clone(), attempt))
    }

    async fn correction(
        &self,
        prior: Self::Attempt,
        message: &str,
    ) -> Result<(String, Self::Attempt), PlanError> {
        let attempt = HttpAnthropicPlanner::correction(self, &prior, message).await?;
        Ok((attempt.text.clone(), attempt))
    }
}

impl crate::planner::CutPlanner for HttpAnthropicPlanner {
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
    fn request_body_matches_messages_api() {
        let body = request_body("claude-sonnet-4-6", "SYS", &[("user".into(), "T".into())]);
        assert_eq!(body["model"], "claude-sonnet-4-6");
        assert_eq!(body["max_tokens"], 8192);
        assert_eq!(body["system"], "SYS");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "T");
    }

    #[test]
    fn debug_redacts_the_key() {
        let p = HttpAnthropicPlanner {
            api_key: "sk-secret".into(),
            model: DEFAULT_MODEL.into(),
            base_url: "http://x".into(),
        };
        let dbg = format!("{p:?}");
        assert!(!dbg.contains("sk-secret"));
    }

    #[tokio::test]
    async fn first_posts_and_extracts_text() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/v1/messages"))
            .and(wiremock::matchers::header("x-api-key", "sk-test"))
            .and(wiremock::matchers::header(
                "anthropic-version",
                "2023-06-01",
            ))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "content": [{"type": "text", "text": "{\"ok\":1}"}],
                    "stop_reason": "end_turn"
                })),
            )
            .expect(1)
            .mount(&server)
            .await;
        let p = HttpAnthropicPlanner {
            api_key: "sk-test".into(),
            model: DEFAULT_MODEL.into(),
            base_url: server.uri(),
        };
        let a = p.first("TRANSCRIPT").await.unwrap();
        assert_eq!(a.text, "{\"ok\":1}");
        assert_eq!(
            a.messages,
            vec![("user".to_string(), "TRANSCRIPT".to_string())]
        );
    }

    #[tokio::test]
    async fn auth_error_is_typed() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(401).set_body_json(serde_json::json!({
                    "type": "error", "error": {"type": "authentication_error", "message": "bad key"}
                })),
            )
            .mount(&server)
            .await;
        let p = HttpAnthropicPlanner {
            api_key: "bad".into(),
            model: DEFAULT_MODEL.into(),
            base_url: server.uri(),
        };
        assert!(matches!(p.first("T").await, Err(PlanError::Auth(_))));
    }

    #[tokio::test]
    async fn correction_appends_assistant_and_user_turns() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::body_partial_json(serde_json::json!({
                "messages": [
                    {"role": "user", "content": "T"},
                    {"role": "assistant", "content": "BAD"},
                    {"role": "user", "content": "fix it"}
                ]
            })))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "content": [{"type": "text", "text": "GOOD"}]
                })),
            )
            .expect(1)
            .mount(&server)
            .await;
        let p = HttpAnthropicPlanner {
            api_key: "k".into(),
            model: DEFAULT_MODEL.into(),
            base_url: server.uri(),
        };
        let prior = HttpAttempt {
            text: "BAD".into(),
            messages: vec![("user".into(), "T".into())],
        };
        let a = p.correction(&prior, "fix it").await.unwrap();
        assert_eq!(a.text, "GOOD");
    }
}
