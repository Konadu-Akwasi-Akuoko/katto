use std::path::PathBuf;

/// Everything needed to assemble one `claude` invocation for a dock session.
pub struct LaunchSpec {
    pub claude_path: PathBuf,
    pub cwd: PathBuf,
    pub initial_prompt: Option<String>,
    pub append_system_prompt: Option<String>,
    pub settings_path: PathBuf,
    pub permission_mode: Option<String>,
}

/// Single-quote shell escaping: wrap in `'…'`, embedded `'` becomes `'\''`.
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// The per-session `--settings` JSON that wires Claude's `Stop`/`Notification`
/// hooks to katto's localhost endpoint. Hooks are shell-form `command` entries
/// because the hook payload arrives on stdin and must be piped verbatim to
/// curl; `|| true` guarantees a dead endpoint never breaks the session. The
/// katto session id is baked into the URL path so the endpoint never has to
/// map Claude-internal session ids.
pub fn hook_settings_json(
    endpoint_port: u16,
    token: &str,
    session_id: &str,
    permission_allow: &[String],
) -> String {
    let curl = format!(
        "curl -s -m 3 -X POST -H 'x-katto-token: {token}' --data-binary @- \
         http://127.0.0.1:{endpoint_port}/hook/{session_id} || true"
    );
    let hook = serde_json::json!({ "type": "command", "command": curl });
    let mut settings = serde_json::json!({
        "hooks": {
            "Stop": [{ "hooks": [hook.clone()] }],
            "Notification": [{
                "matcher": "permission_prompt|idle_prompt",
                "hooks": [hook],
            }],
        },
    });
    if !permission_allow.is_empty() {
        settings["permissions"] = serde_json::json!({ "allow": permission_allow });
    }
    // Serialization of a just-built Value cannot fail; fall back to "{}" anyway
    // rather than panicking in a library path.
    serde_json::to_string_pretty(&settings).unwrap_or_else(|_| "{}".to_string())
}

/// The single string handed to `zsh -lc`: `exec` keeps the PTY child pid equal
/// to the claude process, and the login shell provides the owner's PATH and
/// subscription auth environment. Every operand is `sh_quote`d.
pub fn shell_invocation(spec: &LaunchSpec) -> String {
    let mut line = format!(
        "exec {} --settings {}",
        sh_quote(&spec.claude_path.to_string_lossy()),
        sh_quote(&spec.settings_path.to_string_lossy()),
    );
    if let Some(mode) = &spec.permission_mode {
        line.push_str(&format!(" --permission-mode {}", sh_quote(mode)));
    }
    if let Some(sys) = &spec.append_system_prompt {
        line.push_str(&format!(" --append-system-prompt {}", sh_quote(sys)));
    }
    if let Some(prompt) = &spec.initial_prompt {
        line.push_str(&format!(" {}", sh_quote(prompt)));
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn sh_quote_wraps_and_escapes_single_quotes() {
        assert_eq!(sh_quote("plain"), "'plain'");
        assert_eq!(sh_quote("it's"), r#"'it'\''s'"#);
        assert_eq!(sh_quote("a b; rm -rf /"), "'a b; rm -rf /'");
    }

    #[test]
    fn hook_settings_json_carries_both_hooks_and_token() {
        let json = hook_settings_json(43111, "tok-abc", "sess-1", &[]);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let stop_cmd = v["hooks"]["Stop"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert!(stop_cmd.contains("http://127.0.0.1:43111/hook/sess-1"));
        assert!(stop_cmd.contains("x-katto-token: tok-abc"));
        assert!(stop_cmd.ends_with("|| true"));
        let notif = &v["hooks"]["Notification"][0];
        assert_eq!(notif["matcher"], "permission_prompt|idle_prompt");
        assert!(v.get("permissions").is_none());
    }

    #[test]
    fn hook_settings_json_includes_allow_rules_when_given() {
        let json = hook_settings_json(1, "t", "s", &["Bash(sqlite3:*)".to_string()]);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["permissions"]["allow"][0], "Bash(sqlite3:*)");
    }

    #[test]
    fn shell_invocation_orders_flags_and_quotes_prompt() {
        let spec = LaunchSpec {
            claude_path: PathBuf::from("/usr/local/bin/claude"),
            cwd: PathBuf::from("/tmp"),
            initial_prompt: Some("plan the cut; it's due".into()),
            append_system_prompt: Some("sys".into()),
            settings_path: PathBuf::from("/data/sessions/s1.settings.json"),
            permission_mode: Some("acceptEdits".into()),
        };
        let line = shell_invocation(&spec);
        assert!(line.starts_with(
            "exec '/usr/local/bin/claude' --settings '/data/sessions/s1.settings.json'"
        ));
        assert!(line.contains("--permission-mode 'acceptEdits'"));
        assert!(line.contains("--append-system-prompt 'sys'"));
        assert!(line.ends_with(r#"'plan the cut; it'\''s due'"#));
    }

    #[test]
    fn shell_invocation_minimal_has_no_optional_flags() {
        let spec = LaunchSpec {
            claude_path: PathBuf::from("/x/claude"),
            cwd: PathBuf::from("/tmp"),
            initial_prompt: None,
            append_system_prompt: None,
            settings_path: PathBuf::from("/d/s.json"),
            permission_mode: None,
        };
        let line = shell_invocation(&spec);
        assert_eq!(line, "exec '/x/claude' --settings '/d/s.json'");
    }
}
