//! Pure download-filing rules: where a finished download files, what its
//! license sidecar says, and how name collisions dedupe. No I/O here — the
//! filing job owns the moves.

/// A download in flight: registered at `Requested` time (when katto assigns
/// the staging destination), consumed at `Finished` — macOS never reports the
/// finished path, so this registry entry is the only record of where it went.
#[derive(Clone, Debug)]
pub struct PendingDownload {
    pub id: String,
    pub url: String,
    pub page_url: String,
    pub filename: String,
    pub staging_path: std::path::PathBuf,
    pub started_at: String,
}

/// Strip directory components and leading dots from an untrusted filename;
/// degenerate input becomes "download".
pub fn safe_filename(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or(raw);
    let trimmed = base.trim_start_matches('.').trim();
    if trimmed.is_empty() {
        "download".to_string()
    } else {
        trimmed.to_string()
    }
}

/// True when the URL's host is envato.com or lives under .envato.com /
/// .envatousercontent.com (host-boundary suffix, never substring).
pub fn is_envato_host(url: &url::Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    host == "envato.com"
        || host == "envatousercontent.com"
        || host.ends_with(".envato.com")
        || host.ends_with(".envatousercontent.com")
}

#[derive(Debug, PartialEq)]
pub enum FilingKind {
    Archive,
    File,
}

/// Where a download files and under what names. `dest_dir_rel` is relative to
/// the project dir; `item_name` is the unzip folder (archives) or the final
/// filename (plain files); the sidecar sits beside it.
#[derive(Debug, PartialEq)]
pub struct FilingPlan {
    pub kind: FilingKind,
    pub dest_dir_rel: &'static str,
    pub item_name: String,
    pub sidecar_name: String,
}

/// Decide the filing plan from the download URL, the page it came from, and
/// the safe filename. Envato routing keys off either URL (D-locked #9).
pub fn plan_filing(download_url: &url::Url, page_url: &str, filename: &str) -> FilingPlan {
    let envato =
        is_envato_host(download_url) || url::Url::parse(page_url).is_ok_and(|p| is_envato_host(&p));
    let dest_dir_rel = if envato { "assets/envato" } else { "assets" };
    if is_archive(filename) {
        let stem = &filename[..filename.len() - ".zip".len()];
        FilingPlan {
            kind: FilingKind::Archive,
            dest_dir_rel,
            item_name: stem.to_string(),
            sidecar_name: format!("{stem}.license.json"),
        }
    } else {
        FilingPlan {
            kind: FilingKind::File,
            dest_dir_rel,
            item_name: filename.to_string(),
            sidecar_name: format!("{filename}.license.json"),
        }
    }
}

/// License sidecar JSON: provenance of the asset, serde-built so escaping is
/// never hand-rolled. `note` appears only when present (e.g. an unzip failure).
pub fn license_sidecar_json(
    item_url: &str,
    page_url: &str,
    downloaded_at: &str,
    project_slug: &str,
    note: Option<&str>,
) -> String {
    let mut map = serde_json::Map::new();
    map.insert("item_url".into(), item_url.into());
    map.insert("page_url".into(), page_url.into());
    map.insert("downloaded_at".into(), downloaded_at.into());
    map.insert("project".into(), project_slug.into());
    if let Some(note) = note {
        map.insert("note".into(), note.into());
    }
    let mut json = serde_json::to_string_pretty(&serde_json::Value::Object(map))
        .unwrap_or_else(|_| "{}".to_string());
    json.push('\n');
    json
}

/// First free name: `name`, then `name-2`, `name-3`, … with the counter
/// inserted before the extension when there is one.
pub fn dedupe_name(exists: impl Fn(&str) -> bool, name: &str) -> String {
    if !exists(name) {
        return name.to_string();
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], Some(&name[i..])),
        _ => (name, None),
    };
    let mut n: u64 = 2;
    loop {
        let candidate = match ext {
            Some(ext) => format!("{stem}-{n}{ext}"),
            None => format!("{stem}-{n}"),
        };
        if !exists(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// Only `.zip` counts — Envato Elements delivers zips; everything else files
/// as a plain asset.
pub fn is_archive(filename: &str) -> bool {
    filename.len() > 4 && filename[filename.len() - 4..].eq_ignore_ascii_case(".zip")
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[test]
    fn safe_filename_strips_paths_and_dots() {
        assert_eq!(safe_filename("pack.zip"), "pack.zip");
        assert_eq!(safe_filename("../../etc/passwd"), "passwd");
        assert_eq!(safe_filename(".hidden"), "hidden");
        assert_eq!(safe_filename(""), "download");
        assert_eq!(safe_filename("a/b/c.mov"), "c.mov");
    }

    #[test]
    fn envato_hosts_route_to_envato_dir() {
        for u in [
            "https://elements.envato.com/x",
            "https://envato.com/item",
            "https://dl.envatousercontent.com/signed/abc.zip",
        ] {
            assert!(is_envato_host(&Url::parse(u).unwrap()), "{u}");
        }
        assert!(!is_envato_host(
            &Url::parse("https://notenvato.com/x").unwrap()
        ));
        assert!(!is_envato_host(
            &Url::parse("https://envato.com.evil.tld/x").unwrap()
        ));
    }

    #[test]
    fn archive_plan_names_item_dir_after_stem() {
        let plan = plan_filing(
            &Url::parse("https://dl.envatousercontent.com/x/dust-overlay.zip").unwrap(),
            "https://elements.envato.com/dust-overlay-ABC",
            "dust-overlay.zip",
        );
        assert_eq!(plan.kind, FilingKind::Archive);
        assert_eq!(plan.dest_dir_rel, "assets/envato");
        assert_eq!(plan.item_name, "dust-overlay");
        assert_eq!(plan.sidecar_name, "dust-overlay.license.json");
    }

    #[test]
    fn plain_file_plan_from_non_envato_source() {
        let plan = plan_filing(
            &Url::parse("https://cdn.example.com/lut.cube").unwrap(),
            "https://example.com/luts",
            "lut.cube",
        );
        assert_eq!(plan.kind, FilingKind::File);
        assert_eq!(plan.dest_dir_rel, "assets");
        assert_eq!(plan.item_name, "lut.cube");
        assert_eq!(plan.sidecar_name, "lut.cube.license.json");
    }

    #[test]
    fn envato_page_url_routes_even_when_cdn_is_foreign() {
        let plan = plan_filing(
            &Url::parse("https://cdn.other.net/abc.zip").unwrap(),
            "https://elements.envato.com/thing",
            "abc.zip",
        );
        assert_eq!(plan.dest_dir_rel, "assets/envato");
    }

    #[test]
    fn sidecar_json_shape() {
        let json = license_sidecar_json(
            "https://dl.envatousercontent.com/x.zip",
            "https://elements.envato.com/thing",
            "2026-07-22T12:00:00Z",
            "sprint-recap",
            None,
        );
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["item_url"], "https://dl.envatousercontent.com/x.zip");
        assert_eq!(v["page_url"], "https://elements.envato.com/thing");
        assert_eq!(v["downloaded_at"], "2026-07-22T12:00:00Z");
        assert_eq!(v["project"], "sprint-recap");
        assert!(v.get("note").is_none());
        let with_note = license_sidecar_json("u", "p", "t", "s", Some("unzip_failed: bad magic"));
        let v2: serde_json::Value = serde_json::from_str(&with_note).unwrap();
        assert_eq!(v2["note"], "unzip_failed: bad magic");
    }

    #[test]
    fn dedupe_name_inserts_counter_before_extension() {
        let taken = |n: &str| n == "pack.zip" || n == "pack-2.zip";
        assert_eq!(dedupe_name(taken, "pack.zip"), "pack-3.zip");
        let free = |_: &str| false;
        assert_eq!(dedupe_name(free, "pack.zip"), "pack.zip");
        let dir_taken = |n: &str| n == "dust-overlay";
        assert_eq!(dedupe_name(dir_taken, "dust-overlay"), "dust-overlay-2");
    }

    #[test]
    fn only_zip_is_an_archive() {
        assert!(is_archive("a.zip"));
        assert!(is_archive("A.ZIP"));
        assert!(!is_archive("a.rar"));
        assert!(!is_archive("a.tar.gz"));
        assert!(!is_archive("zip"));
    }
}
