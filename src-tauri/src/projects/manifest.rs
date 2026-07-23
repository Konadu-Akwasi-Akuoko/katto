use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// The manifest schema version this build understands. `read_manifest` rejects
/// any manifest whose `schema_version` is greater than this (a file written by a
/// newer katto). Additive future versions are handled by not denying unknown
/// fields on deserialization, so an older field set still parses.
pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

/// Canonical manifest filename inside a project folder.
pub const MANIFEST_FILE: &str = "project.json";

/// Temporary sibling written before the atomic rename into `MANIFEST_FILE`.
const MANIFEST_TMP: &str = "project.json.tmp";

/// The on-disk project descriptor. Folders are truth; this file is the
/// per-project record the reconcile index is built from. Unknown fields are
/// intentionally NOT denied so a manifest written by a later (additive) schema
/// version still deserializes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub schema_version: u32,
    pub slug: String,
    pub title: String,
    /// `idea | shooting | editing | published` (v1 vocabulary).
    pub status: String,
    pub target_nle: String,
    /// `none | low | medium | high`. Absent in manifests written before this
    /// field existed — the project simply has no priority, which is `none`.
    #[serde(default)]
    pub priority: Option<String>,
    /// `unset | long | short | series` — the video kind carried over from the
    /// idea this project was promoted from. Absent in manifests written before
    /// this field existed, which reads back as `unset`.
    #[serde(default)]
    pub kind: Option<String>,
    pub shoot_date: Option<String>,
    pub publish_date: Option<String>,
    pub created_at: String,
    pub links: serde_json::Map<String, serde_json::Value>,
}

/// Read and validate `<project_dir>/project.json`.
///
/// # Errors
/// - `Error::Io` if the file cannot be read.
/// - `Error::InvalidManifest` if the JSON is malformed, the `schema_version`
///   exceeds this build's `MANIFEST_SCHEMA_VERSION`, or `slug` does not match
///   the directory's own name (folders are truth — a manifest that disagrees
///   with its folder is not trusted).
pub fn read_manifest(project_dir: &Path) -> Result<ProjectManifest> {
    let path = project_dir.join(MANIFEST_FILE);
    let bytes = std::fs::read(&path)?;
    let manifest: ProjectManifest = serde_json::from_slice(&bytes)
        .map_err(|e| Error::invalid_manifest(&path, &e.to_string()))?;
    validate(&manifest, project_dir)?;
    Ok(manifest)
}

/// Atomically write `manifest` to `<project_dir>/project.json`.
///
/// Validates the manifest against `project_dir` (same checks as read), writes to
/// a `.tmp` sibling, then renames it over the target — atomic on a single
/// filesystem (both paths live under `project_dir`). On a serialization or write
/// failure the `.tmp` is cleaned up so no partial file lingers.
///
/// # Errors
/// - `Error::InvalidManifest` if the manifest fails validation against the dir.
/// - `Error::Io` if serialization, the temp write, or the rename fails.
pub fn write_manifest(project_dir: &Path, manifest: &ProjectManifest) -> Result<()> {
    validate(manifest, project_dir)?;
    let bytes = serde_json::to_vec_pretty(manifest).map_err(|e| Error::Io(e.to_string()))?;
    let tmp = project_dir.join(MANIFEST_TMP);
    if let Err(e) = std::fs::write(&tmp, &bytes) {
        let _ = std::fs::remove_file(&tmp);
        return Err(Error::Io(e.to_string()));
    }
    if let Err(e) = std::fs::rename(&tmp, project_dir.join(MANIFEST_FILE)) {
        let _ = std::fs::remove_file(&tmp);
        return Err(Error::Io(e.to_string()));
    }
    Ok(())
}

/// Shared read/write validation: schema version within range and slug agrees
/// with the directory name.
fn validate(manifest: &ProjectManifest, project_dir: &Path) -> Result<()> {
    let path: PathBuf = project_dir.join(MANIFEST_FILE);
    if manifest.schema_version > MANIFEST_SCHEMA_VERSION {
        return Err(Error::invalid_manifest(
            &path,
            &format!(
                "schema_version {} is newer than supported {}",
                manifest.schema_version, MANIFEST_SCHEMA_VERSION
            ),
        ));
    }
    let dir_name = project_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    match dir_name {
        Some(name) if name == manifest.slug => Ok(()),
        Some(name) => Err(Error::invalid_manifest(
            &path,
            &format!("slug `{}` does not match folder `{name}`", manifest.slug),
        )),
        None => Err(Error::invalid_manifest(
            &path,
            "project directory has no file name",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(slug: &str) -> ProjectManifest {
        let mut links = serde_json::Map::new();
        links.insert(
            "youtube".to_string(),
            serde_json::Value::String("https://youtu.be/x".to_string()),
        );
        ProjectManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            slug: slug.to_string(),
            title: "NVMe Deep Dive".to_string(),
            status: "idea".to_string(),
            target_nle: "resolve".to_string(),
            priority: None,
            kind: None,
            shoot_date: Some("2026-07-12".to_string()),
            publish_date: None,
            created_at: "2026-07-09T10:00:00Z".to_string(),
            links,
        }
    }

    fn project_dir(slug: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let proj = dir.path().join(slug);
        std::fs::create_dir(&proj).unwrap();
        (dir, proj)
    }

    #[test]
    fn write_then_read_round_trips() {
        let (_guard, proj) = project_dir("nvme-deep-dive-2026-07-09");
        let manifest = sample("nvme-deep-dive-2026-07-09");
        write_manifest(&proj, &manifest).unwrap();
        let read = read_manifest(&proj).unwrap();
        assert_eq!(read, manifest);
    }

    #[test]
    fn write_leaves_no_tmp_behind() {
        let (_guard, proj) = project_dir("nvme-deep-dive-2026-07-09");
        write_manifest(&proj, &sample("nvme-deep-dive-2026-07-09")).unwrap();
        assert!(proj.join(MANIFEST_FILE).exists());
        assert!(!proj.join(MANIFEST_TMP).exists());
    }

    #[test]
    fn read_rejects_future_schema_version() {
        let (_guard, proj) = project_dir("nvme-deep-dive-2026-07-09");
        let mut manifest = sample("nvme-deep-dive-2026-07-09");
        manifest.schema_version = MANIFEST_SCHEMA_VERSION + 1;
        // Write raw JSON (write_manifest would reject the future version too).
        let bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        std::fs::write(proj.join(MANIFEST_FILE), &bytes).unwrap();
        let err = read_manifest(&proj).unwrap_err();
        assert!(matches!(err, Error::InvalidManifest(_)), "got {err:?}");
    }

    #[test]
    fn read_rejects_slug_dir_mismatch() {
        let (_guard, proj) = project_dir("nvme-deep-dive-2026-07-09");
        // Manifest claims a different slug than its folder name.
        let manifest = sample("some-other-slug-2026-07-09");
        let bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        std::fs::write(proj.join(MANIFEST_FILE), &bytes).unwrap();
        let err = read_manifest(&proj).unwrap_err();
        assert!(matches!(err, Error::InvalidManifest(_)), "got {err:?}");
    }

    #[test]
    fn read_rejects_malformed_json() {
        let (_guard, proj) = project_dir("nvme-deep-dive-2026-07-09");
        std::fs::write(proj.join(MANIFEST_FILE), b"{ not json").unwrap();
        let err = read_manifest(&proj).unwrap_err();
        assert!(matches!(err, Error::InvalidManifest(_)), "got {err:?}");
    }

    #[test]
    fn manifest_without_priority_still_parses() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("legacy-2026-07-16");
        std::fs::create_dir_all(&project_dir).unwrap();
        std::fs::write(
            project_dir.join(MANIFEST_FILE),
            r#"{"schema_version":1,"slug":"legacy-2026-07-16","title":"Legacy",
                "status":"idea","target_nle":"fcp","shoot_date":null,
                "publish_date":null,"created_at":"2026-07-01T00:00:00Z","links":{}}"#,
        )
        .unwrap();
        let m = read_manifest(&project_dir).unwrap();
        assert_eq!(m.priority, None);
    }

    #[test]
    fn unknown_fields_are_accepted_for_forward_compat() {
        let (_guard, proj) = project_dir("nvme-deep-dive-2026-07-09");
        let json = serde_json::json!({
            "schema_version": 1,
            "slug": "nvme-deep-dive-2026-07-09",
            "title": "NVMe Deep Dive",
            "status": "idea",
            "target_nle": "resolve",
            "shoot_date": null,
            "publish_date": null,
            "created_at": "2026-07-09T10:00:00Z",
            "links": {},
            "some_future_field": "ignored"
        });
        std::fs::write(
            proj.join(MANIFEST_FILE),
            serde_json::to_vec_pretty(&json).unwrap(),
        )
        .unwrap();
        let read = read_manifest(&proj).unwrap();
        assert_eq!(read.slug, "nvme-deep-dive-2026-07-09");
    }
}
