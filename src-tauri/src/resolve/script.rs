//! Pure generation and parsing for the Resolve scripting bridge: the Python
//! source katto spawns, and the single stdout JSON line it answers with.

use std::path::Path;

/// The bridge's outcome, decoded from the script's stdout.
#[derive(Debug, PartialEq)]
pub enum ScriptOutcome {
    Ok { project: String },
    NotConnected,
    CreateFailed(String),
    ImportFailed(String),
    Garbled(String),
}

/// The Python the bridge runs: create a Resolve project named after the
/// export and import the FCPXML (which also populates the media pool).
/// Names embed via serde_json string encoding — never format-string escaping.
pub fn import_script(project_name: &str, fcpxml_path: &Path) -> String {
    let name_json = serde_json::to_string(project_name).unwrap_or_else(|_| "\"\"".to_string());
    let path_json = serde_json::to_string(&fcpxml_path.to_string_lossy())
        .unwrap_or_else(|_| "\"\"".to_string());
    format!(
        r#"import json, sys
try:
    import DaVinciResolveScript as dvr
except Exception as e:
    print(json.dumps({{"error": "not_connected", "detail": str(e)}})); sys.exit(0)
resolve = dvr.scriptapp("Resolve")
if resolve is None:
    print(json.dumps({{"error": "not_connected"}})); sys.exit(0)
pm = resolve.GetProjectManager()
name = {name_json}
project = pm.CreateProject(name)
if project is None:
    # name collision - retry once with a suffix
    project = pm.CreateProject(name + " (katto)")
if project is None:
    print(json.dumps({{"error": "create_failed", "detail": name}})); sys.exit(0)
pool = project.GetMediaPool()
timeline = pool.ImportTimelineFromFile({path_json})
if timeline is None:
    print(json.dumps({{"error": "import_failed", "detail": "ImportTimelineFromFile returned None"}})); sys.exit(0)
print(json.dumps({{"ok": True, "project": name}}))
"#
    )
}

/// Decode stdout: the last line that parses as a JSON object decides
/// (Resolve's own stdout noise may precede it); nothing parseable → Garbled.
pub fn parse_outcome(stdout: &str) -> ScriptOutcome {
    for line in stdout.lines().rev() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line.trim()) else {
            continue;
        };
        if !value.is_object() {
            continue;
        }
        if value.get("ok").and_then(|v| v.as_bool()) == Some(true) {
            let project = value
                .get("project")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            return ScriptOutcome::Ok { project };
        }
        let detail = value
            .get("detail")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        return match value.get("error").and_then(|v| v.as_str()) {
            Some("not_connected") => ScriptOutcome::NotConnected,
            Some("create_failed") => ScriptOutcome::CreateFailed(detail),
            Some("import_failed") => ScriptOutcome::ImportFailed(detail),
            _ => ScriptOutcome::Garbled(line.trim().chars().take(200).collect()),
        };
    }
    ScriptOutcome::Garbled(stdout.chars().take(200).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn script_embeds_project_and_fcpxml_path() {
        let s = import_script(
            "sprint-recap v3",
            Path::new("/Volumes/SSD/Projects/sprint-recap/timelines/sprint-recap-v3.fcpxml"),
        );
        assert!(s.contains("import DaVinciResolveScript"));
        assert!(s.contains(r#"scriptapp("Resolve")"#));
        assert!(s.contains(r#""sprint-recap v3""#));
        assert!(
            s.contains(r#""/Volumes/SSD/Projects/sprint-recap/timelines/sprint-recap-v3.fcpxml""#)
        );
        assert!(s.contains("ImportTimelineFromFile"));
        assert!(s.contains("not_connected"));
    }

    #[test]
    fn script_escapes_quotes_in_names() {
        let s = import_script(r#"the "big" one"#, Path::new("/tmp/a.fcpxml"));
        assert!(s.contains(r#""the \"big\" one""#));
    }

    #[test]
    fn parse_outcome_variants() {
        assert_eq!(
            parse_outcome(r#"{"ok": true, "project": "p v3"}"#),
            ScriptOutcome::Ok {
                project: "p v3".into()
            }
        );
        assert_eq!(
            parse_outcome(r#"{"error": "not_connected"}"#),
            ScriptOutcome::NotConnected
        );
        assert_eq!(
            parse_outcome(r#"{"error": "import_failed", "detail": "bad xml"}"#),
            ScriptOutcome::ImportFailed("bad xml".into())
        );
        assert_eq!(
            parse_outcome(r#"{"error": "create_failed", "detail": "duplicate"}"#),
            ScriptOutcome::CreateFailed("duplicate".into())
        );
        assert!(matches!(
            parse_outcome("Traceback (most recent call last)…"),
            ScriptOutcome::Garbled(_)
        ));
        // outcome line may be preceded by Resolve's own stdout noise
        assert_eq!(
            parse_outcome("noise\n{\"ok\": true, \"project\": \"x\"}\n"),
            ScriptOutcome::Ok {
                project: "x".into()
            }
        );
    }
}
