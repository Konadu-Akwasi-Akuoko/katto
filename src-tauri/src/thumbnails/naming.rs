//! Pure naming rules for thumbnail scaffolds: spreadsheet letters (a…z, aa,
//! ab…) that never collide with existing files, and the newest-PNG pick the
//! cards render.

/// Decode spreadsheet letters as bijective base-26 (`a`=1 … `z`=26, `aa`=27).
fn decode_letters(letters: &str) -> Option<u64> {
    if letters.is_empty() {
        return None;
    }
    let mut acc: u64 = 0;
    for ch in letters.chars() {
        if !ch.is_ascii_lowercase() {
            return None;
        }
        acc = acc
            .checked_mul(26)?
            .checked_add(u64::from(ch as u8 - b'a') + 1)?;
    }
    Some(acc)
}

fn encode_letters(mut n: u64) -> String {
    let mut out = Vec::new();
    while n > 0 {
        n -= 1;
        out.push(b'a' + (n % 26) as u8);
        n /= 26;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_else(|_| "a".to_string())
}

/// The next free `<slug>-thumb-<letters>.<ext>` name: max existing letter
/// index for this slug/ext plus one. Existing names match case-insensitively
/// — APFS is case-insensitive, so `X.PSD` would collide with `x.psd`.
pub fn next_thumb_name(existing: &[String], slug: &str, ext: &str) -> String {
    let prefix = format!("{slug}-thumb-");
    let suffix = format!(".{ext}");
    let max = existing
        .iter()
        .filter_map(|name| {
            let lowered = name.to_lowercase();
            let letters = lowered.strip_prefix(&prefix)?.strip_suffix(&suffix)?;
            decode_letters(letters)
        })
        .max()
        .unwrap_or(0);
    format!("{prefix}{}{suffix}", encode_letters(max + 1))
}

/// The most recently modified `.png` (case-insensitive) among `(name, mtime)`
/// pairs; `None` when there is none.
pub fn newest_png(entries: &[(String, std::time::SystemTime)]) -> Option<String> {
    entries
        .iter()
        .filter(|(name, _)| {
            std::path::Path::new(name)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("png"))
        })
        .max_by_key(|(_, mtime)| *mtime)
        .map(|(name, _)| name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_scaffold_is_letter_a() {
        assert_eq!(
            next_thumb_name(&[], "sprint-recap", "psd"),
            "sprint-recap-thumb-a.psd"
        );
    }

    #[test]
    fn letters_increment_past_gaps() {
        let existing = vec![
            "sprint-recap-thumb-a.psd".into(),
            "sprint-recap-thumb-c.psd".into(),
        ];
        assert_eq!(
            next_thumb_name(&existing, "sprint-recap", "psd"),
            "sprint-recap-thumb-d.psd"
        );
    }

    #[test]
    fn after_z_comes_aa() {
        let existing: Vec<String> = ('a'..='z').map(|c| format!("s-thumb-{c}.psd")).collect();
        assert_eq!(next_thumb_name(&existing, "s", "psd"), "s-thumb-aa.psd");
    }

    #[test]
    fn other_slugs_and_files_are_ignored() {
        let existing = vec![
            "other-thumb-a.psd".into(),
            "sprint-recap-thumb-a.png".into(),
            "notes.txt".into(),
        ];
        assert_eq!(
            next_thumb_name(&existing, "sprint-recap", "psd"),
            "sprint-recap-thumb-a.psd"
        );
    }

    #[test]
    fn existing_names_match_case_insensitively() {
        // APFS is case-insensitive: an uppercased survivor must still count,
        // or the next scaffold would collide with it on disk
        let existing = vec!["Sprint-Recap-THUMB-B.PSD".into()];
        assert_eq!(
            next_thumb_name(&existing, "sprint-recap", "psd"),
            "sprint-recap-thumb-c.psd"
        );
    }

    #[test]
    fn newest_png_picks_latest_mtime_png_only() {
        use std::time::{Duration, SystemTime};
        let t0 = SystemTime::UNIX_EPOCH;
        let entries = vec![
            ("a.png".to_string(), t0 + Duration::from_secs(10)),
            ("b.PNG".to_string(), t0 + Duration::from_secs(20)),
            ("c.psd".to_string(), t0 + Duration::from_secs(30)),
        ];
        assert_eq!(newest_png(&entries), Some("b.PNG".to_string()));
        assert_eq!(newest_png(&[]), None);
    }
}
