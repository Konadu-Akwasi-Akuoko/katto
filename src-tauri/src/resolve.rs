//! DaVinci Resolve Studio bridge: preflights, the python3 spawn with the
//! scripting env, and outcome mapping. Script text and stdout parsing are
//! pure (`resolve/script.rs`); this file is the thin spawn site. katto never
//! launches Resolve — the owner opens it (PRD D12).

pub mod script;

use std::io::Write;
use std::path::Path;
use std::time::Duration;

use crate::error::{Error, Result};

const RESOLVE_APP: &str = "/Applications/DaVinci Resolve/DaVinci Resolve.app";
const SCRIPT_API: &str =
    "/Library/Application Support/Blackmagic Design/DaVinci Resolve/Developer/Scripting";
const SCRIPT_LIB: &str =
    "/Applications/DaVinci Resolve/DaVinci Resolve.app/Contents/Libraries/Fusion/fusionscript.so";
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(60);

/// Resolve's app bundle is present (any edition).
pub fn resolve_installed() -> bool {
    Path::new(RESOLVE_APP).exists()
}

/// A Resolve process is running (`pgrep -x` on both observed process names).
pub fn resolve_running() -> bool {
    ["DaVinci Resolve", "Resolve"].iter().any(|name| {
        std::process::Command::new("pgrep")
            .args(["-x", name])
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    })
}

/// The three env vars external Resolve scripting requires.
pub fn python_env() -> [(&'static str, String); 3] {
    [
        ("RESOLVE_SCRIPT_API", SCRIPT_API.to_string()),
        ("RESOLVE_SCRIPT_LIB", SCRIPT_LIB.to_string()),
        ("PYTHONPATH", format!("{SCRIPT_API}/Modules/")),
    ]
}

/// Run the import script against the running Resolve and decode its answer.
///
/// # Errors
/// `ResolveFailed` on spawn/timeout/IO problems; outcome mapping is the
/// caller's job (it owns the remedy strings).
pub async fn run_import(project_name: &str, fcpxml_path: &Path) -> Result<script::ScriptOutcome> {
    let source = script::import_script(project_name, fcpxml_path);
    let mut file = tempfile::Builder::new()
        .prefix("katto-resolve-")
        .suffix(".py")
        .tempfile()
        .map_err(|e| Error::ResolveFailed(format!("script temp file: {e}")))?;
    file.write_all(source.as_bytes())
        .map_err(|e| Error::ResolveFailed(format!("script write: {e}")))?;
    let path = file.path().to_path_buf();

    let mut command = tokio::process::Command::new("/usr/bin/env");
    command.arg("python3").arg(&path);
    for (key, value) in python_env() {
        command.env(key, value);
    }
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    let output = tokio::time::timeout(SCRIPT_TIMEOUT, async {
        command
            .spawn()
            .map_err(|e| Error::ResolveFailed(format!("python3 spawn failed: {e}")))?
            .wait_with_output()
            .await
            .map_err(|e| Error::ResolveFailed(format!("python3 wait failed: {e}")))
    })
    .await
    .map_err(|_| Error::ResolveFailed("Resolve scripting timed out after 60 s".to_string()))??;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(script::parse_outcome(&stdout))
}
