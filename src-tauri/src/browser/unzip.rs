//! Slip-safe archive extraction for filed downloads. zip's `extract`
//! sanitizes entry paths via `enclosed_name`; the tests still assert the
//! escape case because the invariant is load-bearing.

use std::path::Path;

/// The single top-level directory every entry lives under, if there is one.
/// Envato zips wrap one folder; filing flattens it away.
pub fn single_root(names: &[String]) -> Option<String> {
    let mut root: Option<&str> = None;
    for name in names {
        let top = name.split('/').next().filter(|t| !t.is_empty())?;
        // a bare top-level file (no '/') means the archive is flat
        if !name.contains('/') {
            return None;
        }
        match root {
            None => root = Some(top),
            Some(r) if r == top => {}
            Some(_) => return None,
        }
    }
    root.map(str::to_string)
}

/// Extract `archive` into `dest_dir` (created if needed), flattening a single
/// top-level directory. Returns the number of extracted file entries.
///
/// # Errors
/// Any zip or I/O failure maps to [`crate::error::Error::UnzipFailed`].
pub fn extract_archive(archive: &Path, dest_dir: &Path) -> crate::error::Result<usize> {
    extract_inner(archive, dest_dir).map_err(|e| crate::error::Error::UnzipFailed(e.to_string()))
}

fn extract_inner(
    archive: &Path,
    dest_dir: &Path,
) -> std::result::Result<usize, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file)?;
    let mut names = Vec::with_capacity(zip.len());
    let mut file_count = 0usize;
    for i in 0..zip.len() {
        let entry = zip.by_index(i)?;
        if !entry.is_dir() {
            file_count += 1;
        }
        names.push(entry.name().to_string());
    }
    std::fs::create_dir_all(dest_dir)?;
    match single_root(&names) {
        Some(root) => {
            let parent = dest_dir.parent().unwrap_or(dest_dir);
            let tmp = tempfile::tempdir_in(parent)?;
            zip.extract(tmp.path())?;
            // dest_dir was just created empty; replace it with the unwrapped root
            std::fs::remove_dir(dest_dir)?;
            std::fs::rename(tmp.path().join(&root), dest_dir)?;
        }
        None => zip.extract(dest_dir)?,
    }
    Ok(file_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(entries: &[(&str, &[u8])]) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut w = zip::ZipWriter::new(file.reopen().unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        for (name, bytes) in entries {
            w.start_file(*name, opts).unwrap();
            w.write_all(bytes).unwrap();
        }
        w.finish().unwrap();
        file
    }

    #[test]
    fn single_root_detects_wrapped_archives() {
        let names = vec!["pack/".into(), "pack/a.mov".into(), "pack/sub/b.png".into()];
        assert_eq!(single_root(&names), Some("pack".to_string()));
        let flat = vec!["a.mov".into(), "b.png".into()];
        assert_eq!(single_root(&flat), None);
        let mixed = vec!["pack/a.mov".into(), "other/b.png".into()];
        assert_eq!(single_root(&mixed), None);
    }

    #[test]
    fn extract_flattens_single_root() {
        let z = make_zip(&[("pack/a.txt", b"a"), ("pack/sub/b.txt", b"b")]);
        let dest = tempfile::tempdir().unwrap();
        let target = dest.path().join("dust");
        let n = extract_archive(z.path(), &target).unwrap();
        assert_eq!(n, 2);
        assert!(target.join("a.txt").is_file());
        assert!(target.join("sub/b.txt").is_file());
        assert!(!target.join("pack").exists());
    }

    #[test]
    fn extract_keeps_flat_archives_flat() {
        let z = make_zip(&[("a.txt", b"a"), ("b/c.txt", b"c")]);
        let dest = tempfile::tempdir().unwrap();
        let target = dest.path().join("out");
        extract_archive(z.path(), &target).unwrap();
        assert!(target.join("a.txt").is_file());
        assert!(target.join("b/c.txt").is_file());
    }

    #[test]
    fn zip_slip_entry_cannot_escape_dest() {
        let z = make_zip(&[("../evil.txt", b"x"), ("ok.txt", b"y")]);
        let outer = tempfile::tempdir().unwrap();
        let target = outer.path().join("inner");
        // zip's extract sanitizes via enclosed_name; whether it skips or errors,
        // the escape file must not exist outside dest.
        let _ = extract_archive(z.path(), &target);
        assert!(!outer.path().join("evil.txt").exists());
    }

    #[test]
    fn garbage_file_is_a_typed_error() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), b"not a zip").unwrap();
        let dest = tempfile::tempdir().unwrap();
        let err = extract_archive(file.path(), dest.path()).unwrap_err();
        assert!(matches!(err, crate::error::Error::UnzipFailed(_)));
    }
}
