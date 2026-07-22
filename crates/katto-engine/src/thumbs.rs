//! Timeline filmstrip thumbnails: one 320px-wide JPEG every 2 seconds,
//! regenerated atomically (`thumbs.tmp/` -> swap) so a crash never leaves a
//! half-written strip.

use std::path::Path;

use crate::error::{Error, Result};

/// Bundle-relative thumbnails directory name.
pub const THUMBS_DIR: &str = "thumbs";

/// Pinned argv: one 320px-wide JPEG every 2 seconds into `<out_dir>/%05d.jpg`.
pub fn thumbs_argv(src: &Path, out_dir: &Path) -> Vec<String> {
    vec![
        "ffmpeg".into(),
        "-nostdin".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
        "-i".into(),
        src.to_string_lossy().into_owned(),
        "-vf".into(),
        "fps=1/2,scale=320:-2".into(),
        "-q:v".into(),
        "5".into(),
        out_dir.join("%05d.jpg").to_string_lossy().into_owned(),
    ]
}

/// Regenerate `<bundle_root>/thumbs/` atomically (render into `thumbs.tmp/`,
/// swap dirs). Returns the frame count. Idempotent.
///
/// # Errors
/// [`Error::Render`] with the ffmpeg stderr tail on extraction failure (the
/// `.tmp` dir is removed); [`Error::Io`] on filesystem failures.
pub async fn generate_thumbs(bundle_root: &Path, src: &Path) -> Result<u32> {
    let final_dir = bundle_root.join(THUMBS_DIR);
    let tmp_dir = bundle_root.join(format!("{THUMBS_DIR}.tmp"));
    if tmp_dir.exists() {
        tokio::fs::remove_dir_all(&tmp_dir).await?;
    }
    tokio::fs::create_dir_all(&tmp_dir).await?;

    let argv = thumbs_argv(src, &tmp_dir);
    let output = tokio::process::Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|e| Error::Render(format!("ffmpeg spawn failed: {e}")))?;
    if !output.status.success() {
        let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
        return Err(Error::Render(format!(
            "thumbnail extraction exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    if final_dir.exists() {
        tokio::fs::remove_dir_all(&final_dir).await?;
    }
    tokio::fs::rename(&tmp_dir, &final_dir).await?;

    let mut count = 0;
    let mut entries = tokio::fs::read_dir(&final_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        if entry.path().extension().is_some_and(|e| e == "jpg") {
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbs_argv_is_pinned() {
        let argv = thumbs_argv(Path::new("/a/clip.mp4"), Path::new("/b/thumbs.tmp"));
        assert_eq!(
            argv,
            vec![
                "ffmpeg",
                "-nostdin",
                "-loglevel",
                "error",
                "-y",
                "-i",
                "/a/clip.mp4",
                "-vf",
                "fps=1/2,scale=320:-2",
                "-q:v",
                "5",
                "/b/thumbs.tmp/%05d.jpg",
            ]
        );
    }

    #[tokio::test]
    #[ignore = "spawns real ffmpeg; owner checkpoint (KATTO_TEST_CLIP)"]
    async fn generate_thumbs_real_clip() {
        let Ok(clip) = std::env::var("KATTO_TEST_CLIP") else {
            eprintln!("KATTO_TEST_CLIP not set; skipping");
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let count = generate_thumbs(dir.path(), Path::new(&clip)).await.unwrap();
        assert!(count > 0);
        assert!(dir.path().join(THUMBS_DIR).join("00001.jpg").exists());
        assert!(!dir.path().join("thumbs.tmp").exists());
    }
}
