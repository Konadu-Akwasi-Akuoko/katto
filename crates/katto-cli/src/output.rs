//! Pure render functions for CLI output — snapshot-tested; `main.rs` only
//! prints what these return.

use std::path::Path;

use katto_engine::import::ImportOutcome;
use katto_engine::schema::Cuts;
use katto_engine::timelines::ExportPaths;

use crate::keys::KeySource;

/// What `katto auth status` reports (no key values, ever).
#[derive(Debug, Clone)]
pub struct AuthStatus {
    /// Detected claude binary path, if any.
    pub claude_path: Option<String>,
    /// Where the ElevenLabs key would come from.
    pub elevenlabs: KeySource,
    /// Where the Anthropic key would come from.
    pub anthropic: KeySource,
}

/// Render `katto auth status`.
pub fn render_auth_status(s: &AuthStatus, json: bool) -> String {
    if json {
        return serde_json::json!({
            "claude_path": s.claude_path,
            "elevenlabs": s.elevenlabs.as_str(),
            "anthropic": s.anthropic.as_str(),
        })
        .to_string();
    }
    let claude = match &s.claude_path {
        Some(p) => format!("claude: {p} (subscription auth)"),
        None => "claude: not found (BYOK http planner only)".to_string(),
    };
    format!(
        "{claude}\nelevenlabs key: {}\nanthropic key: {}",
        s.elevenlabs.as_str(),
        s.anthropic.as_str()
    )
}

/// Render `katto import` success.
pub fn render_import(o: &ImportOutcome, json: bool) -> String {
    if json {
        return serde_json::json!({
            "bundle": o.bundle_root,
            "frame_rate": {"num": o.manifest.frame_rate.num, "den": o.manifest.frame_rate.den},
            "duration_secs": o.manifest.duration.to_secs_f64(),
        })
        .to_string();
    }
    format!(
        "imported {} ({:.3}s @ {}/{})",
        o.bundle_root.display(),
        o.manifest.duration.to_secs_f64(),
        o.manifest.frame_rate.num,
        o.manifest.frame_rate.den
    )
}

/// Render `katto transcribe` success.
pub fn render_transcribe(bundle: &Path, words: usize, json: bool) -> String {
    if json {
        return serde_json::json!({"bundle": bundle, "tokens": words}).to_string();
    }
    format!("transcribed {} ({words} tokens)", bundle.display())
}

/// Render `katto plan` success.
pub fn render_plan(bundle: &Path, cuts: &Cuts, json: bool) -> String {
    if json {
        return serde_json::json!({
            "bundle": bundle,
            "cuts": cuts.cuts.len(),
            "discretionary": cuts.discretionary.len(),
            "flags": cuts.flags.len(),
            "total_cut_secs": cuts.total_cut_secs,
        })
        .to_string();
    }
    format!(
        "planned {}: {} cuts, {} discretionary, {} flags ({:.3}s cut)",
        bundle.display(),
        cuts.cuts.len(),
        cuts.discretionary.len(),
        cuts.flags.len(),
        cuts.total_cut_secs
    )
}

/// Render `katto export` success.
pub fn render_export(p: &ExportPaths, json: bool) -> String {
    if json {
        return serde_json::json!({
            "fcpxml": p.fcpxml,
            "srt": p.srt,
            "vtt": p.vtt,
            "version": p.version,
        })
        .to_string();
    }
    format!(
        "exported v{}:\n  {}\n  {}\n  {}",
        p.version,
        p.fcpxml.display(),
        p.srt.display(),
        p.vtt.display()
    )
}

/// Render `katto render` success.
pub fn render_render(out: &Path, json: bool) -> String {
    if json {
        return serde_json::json!({ "out": out }).to_string();
    }
    format!("rendered {}", out.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_status_human_render() {
        let s = AuthStatus {
            claude_path: Some("/Users/x/.local/bin/claude".into()),
            elevenlabs: KeySource::Keychain,
            anthropic: KeySource::Missing,
        };
        insta::assert_snapshot!("auth_status_human", render_auth_status(&s, false));
    }

    #[test]
    fn auth_status_json_render() {
        let s = AuthStatus {
            claude_path: None,
            elevenlabs: KeySource::Env,
            anthropic: KeySource::Env,
        };
        insta::assert_snapshot!("auth_status_json", render_auth_status(&s, true));
    }

    #[test]
    fn export_renders_paths_and_version() {
        let p = ExportPaths {
            fcpxml: "/p/timelines/demo-v3.fcpxml".into(),
            srt: "/p/timelines/demo-v3.srt".into(),
            vtt: "/p/timelines/demo-v3.vtt".into(),
            version: 3,
        };
        insta::assert_snapshot!("export_human", render_export(&p, false));
        insta::assert_snapshot!("export_json", render_export(&p, true));
    }

    #[test]
    fn render_render_names_the_output() {
        insta::assert_snapshot!(
            "render_human",
            render_render(Path::new("/p/exports/demo-render-v1.mp4"), false)
        );
        insta::assert_snapshot!(
            "render_json",
            render_render(Path::new("/p/exports/demo-render-v1.mp4"), true)
        );
    }

    #[test]
    fn plan_render_counts() {
        let cuts = Cuts {
            source_duration_secs: 8.0,
            cuts: vec![],
            discretionary: vec![],
            flags: vec![],
            total_cut_secs: 0.0,
        };
        insta::assert_snapshot!(
            "plan_human_empty",
            render_plan(Path::new("/p/clip.kruproj"), &cuts, false)
        );
    }
}
