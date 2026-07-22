//! Thumbnail scaffolds: pure PSD template writer (bundled resources), letter
//! naming, and the PNG folder watch. katto copies templates and shows exported
//! PNGs — it never edits a PSD (D17: no design intelligence).

pub mod naming;
pub mod psd;
pub mod watch;

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager, path::BaseDirectory};

use crate::error::{Error, Result};

/// Which bundled template a scaffold copies.
#[derive(serde::Deserialize, specta::Type, Clone, Copy, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ThumbFormat {
    Landscape,
    Portrait,
}

impl ThumbFormat {
    fn template_file(self) -> &'static str {
        match self {
            ThumbFormat::Landscape => "thumb-1280x720.psd",
            ThumbFormat::Portrait => "thumb-1080x1920.psd",
        }
    }
}

/// Copy the bundled template into `<project>/thumbnails/` under the next
/// letter name. Never overwrites: the copy target opens with `create_new`.
pub fn scaffold(
    app: &AppHandle,
    project_dir: &Path,
    slug: &str,
    format: ThumbFormat,
) -> Result<PathBuf> {
    let template = app
        .path()
        .resolve(
            format!("resources/thumbnail-templates/{}", format.template_file()),
            BaseDirectory::Resource,
        )
        .map_err(|e| Error::Io(format!("bundled template missing: {e}")))?;
    let thumbs_dir = project_dir.join("thumbnails");
    std::fs::create_dir_all(&thumbs_dir)?;
    let existing: Vec<String> = std::fs::read_dir(&thumbs_dir)?
        .flatten()
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .collect();
    let name = naming::next_thumb_name(&existing, slug, "psd");
    let target = thumbs_dir.join(&name);
    let mut source = std::fs::File::open(&template)
        .map_err(|e| Error::Io(format!("bundled template {}: {e}", template.display())))?;
    let mut dest = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|e| Error::Io(format!("scaffold target {}: {e}", target.display())))?;
    std::io::copy(&mut source, &mut dest)?;
    Ok(target)
}

/// How the scaffold opened for the owner.
#[derive(serde::Serialize, specta::Type, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ThumbOpen {
    Photoshop,
    RevealedInFinder,
}

/// Open the scaffold in Photoshop; absent/failed → reveal in Finder instead
/// (PRD: the PSD still scaffolds — Photoshop is never a blocker).
pub fn open_scaffold(app: &AppHandle, path: &Path) -> ThumbOpen {
    // by bundle id, not name: real installs are versioned ("Adobe Photoshop
    // 2026.app"), so `open -a "Adobe Photoshop"` misses them
    let opened = std::process::Command::new("open")
        .args(["-b", "com.adobe.Photoshop"])
        .arg(path)
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if opened {
        return ThumbOpen::Photoshop;
    }
    use tauri_plugin_opener::OpenerExt;
    let _ = app.opener().reveal_item_in_dir(path);
    ThumbOpen::RevealedInFinder
}
