//! Shared environment detection and credential naming. One home so the CLI
//! and the app agree on the keychain layout and the claude probe by
//! construction, not by comment-contract.

use std::path::PathBuf;

/// Keychain service name every katto credential lives under.
pub const KEYCHAIN_SERVICE: &str = "katto";
/// Keychain account name for the ElevenLabs key.
pub const ELEVENLABS_ACCOUNT: &str = "elevenlabs";
/// Keychain account name for the Anthropic key.
pub const ANTHROPIC_ACCOUNT: &str = "anthropic";

/// Extract the binary path from `which` output: success status plus a single
/// absolute-path line, else `None`.
pub fn parse_which_output(success: bool, stdout: &str) -> Option<String> {
    if !success {
        return None;
    }
    let line = stdout.lines().next()?.trim();
    line.starts_with('/').then(|| line.to_string())
}

/// `which claude` through a login shell, so the owner's PATH additions apply.
/// Blocking — call from a blocking context. Not-found is a normal outcome
/// (`None`), never an error.
pub fn detect_claude() -> Option<PathBuf> {
    let out = std::process::Command::new("zsh")
        .args(["-lc", "which claude"])
        .output()
        .ok()?;
    parse_which_output(out.status.success(), &String::from_utf8_lossy(&out.stdout))
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn which_output_parses_only_successful_absolute_paths() {
        assert_eq!(
            parse_which_output(true, "/opt/homebrew/bin/claude\n"),
            Some("/opt/homebrew/bin/claude".to_string())
        );
        assert_eq!(parse_which_output(false, "claude not found\n"), None);
        assert_eq!(parse_which_output(true, ""), None);
        assert_eq!(parse_which_output(true, "claude not found"), None);
    }
}
