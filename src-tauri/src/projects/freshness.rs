use std::path::Path;
use std::time::SystemTime;

use serde::Serialize;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use super::anatomy::PROJECT_SUBFOLDERS;
use crate::error::{Error, Result};

/// Per-subfolder freshness for a project's D6 anatomy: how many files a subfolder
/// holds (non-recursive) and the newest file mtime. `latest_mtime` is `None` for
/// an empty or absent subfolder. Feeds the project-detail "what has activity"
/// grid — never a score, only a raw count and timestamp.
#[derive(Debug, Serialize, specta::Type)]
pub struct FolderFreshness {
    pub subfolder: String,
    pub file_count: u32,
    pub latest_mtime: Option<String>,
}

/// Freshness of every [`PROJECT_SUBFOLDERS`] entry under `project_dir`, in
/// declaration order. A missing subfolder degrades to `0`/`None` rather than
/// erroring — folders are truth and a not-yet-created subfolder is a normal
/// state.
///
/// # Errors
/// `Error::Io` if a directory read or a file-metadata query fails for a reason
/// other than absence.
pub fn folder_freshness(project_dir: &Path) -> Result<Vec<FolderFreshness>> {
    PROJECT_SUBFOLDERS
        .iter()
        .map(|sub| {
            let (file_count, latest_mtime) = dir_freshness(&project_dir.join(sub))?;
            Ok(FolderFreshness {
                subfolder: (*sub).to_string(),
                file_count,
                latest_mtime,
            })
        })
        .collect()
}

/// Non-recursive `(file count, newest file mtime as RFC3339)` for a single
/// directory. An absent directory is `(0, None)`. Only regular files count;
/// subdirectories are ignored for both the count and the mtime.
fn dir_freshness(dir: &Path) -> Result<(u32, Option<String>)> {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((0, None)),
        Err(e) => return Err(Error::Io(e.to_string())),
    };

    let mut count: u32 = 0;
    let mut latest: Option<SystemTime> = None;
    for entry in read_dir {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        count += 1;
        let modified = entry.metadata()?.modified()?;
        latest = Some(match latest {
            Some(current) if current >= modified => current,
            _ => modified,
        });
    }

    let latest_mtime = latest.map(format_rfc3339).transpose()?;
    Ok((count, latest_mtime))
}

/// Render a filesystem `SystemTime` as a UTC RFC3339 string (e.g.
/// `2026-07-09T12:34:56.123456789Z`). Fractional seconds are included when the
/// mtime carries sub-second precision — valid RFC3339 and independent of the
/// second-precision timestamps the events log stamps.
fn format_rfc3339(time: SystemTime) -> Result<String> {
    OffsetDateTime::from(time)
        .format(&Rfc3339)
        .map_err(|e| Error::Io(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_dir_reports_zero_and_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(dir_freshness(dir.path()).unwrap(), (0, None));
    }

    #[test]
    fn absent_dir_reports_zero_and_none() {
        let dir = tempfile::tempdir().unwrap();
        let (count, mtime) = dir_freshness(&dir.path().join("nope")).unwrap();
        assert_eq!(count, 0);
        assert_eq!(mtime, None);
    }

    #[test]
    fn two_files_report_count_two_and_a_timestamp() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.mov"), b"a").unwrap();
        std::fs::write(dir.path().join("b.mov"), b"b").unwrap();
        let (count, mtime) = dir_freshness(dir.path()).unwrap();
        assert_eq!(count, 2);
        let mtime = mtime.expect("a populated dir has a latest mtime");
        assert!(mtime.ends_with('Z'), "RFC3339 UTC, got {mtime}");
    }

    #[test]
    fn subdirectories_are_not_counted_as_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("nested")).unwrap();
        std::fs::write(dir.path().join("clip.mov"), b"x").unwrap();
        assert_eq!(dir_freshness(dir.path()).unwrap().0, 1);
    }

    #[test]
    fn format_rfc3339_renders_utc_with_z() {
        let time = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_752_062_096);
        assert_eq!(format_rfc3339(time).unwrap(), "2025-07-09T11:54:56Z");
    }

    #[test]
    fn folder_freshness_covers_every_subfolder() {
        let dir = tempfile::tempdir().unwrap();
        for sub in PROJECT_SUBFOLDERS {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
        }
        std::fs::write(dir.path().join("footage").join("clip.mov"), b"x").unwrap();

        let fresh = folder_freshness(dir.path()).unwrap();
        assert_eq!(fresh.len(), PROJECT_SUBFOLDERS.len());
        let footage = fresh.iter().find(|f| f.subfolder == "footage").unwrap();
        assert_eq!(footage.file_count, 1);
        assert!(footage.latest_mtime.is_some());
        let audio = fresh.iter().find(|f| f.subfolder == "audio").unwrap();
        assert_eq!(audio.file_count, 0);
        assert_eq!(audio.latest_mtime, None);
        for sub in ["assets/music", "assets/sfx"] {
            assert!(
                fresh.iter().any(|f| f.subfolder == sub),
                "freshness must cover {sub}"
            );
        }
    }
}
