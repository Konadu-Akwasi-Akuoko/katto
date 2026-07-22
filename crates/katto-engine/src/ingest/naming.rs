//! Deterministic footage renaming: `YYYY-MM-DD_NNN.ext` with a per-date
//! sequence continuing from what already exists in `footage/`.

use std::path::PathBuf;

use crate::ingest::Rename;

/// The next sequence for `date`, continuing from the highest existing
/// `YYYY-MM-DD_NNN.*` name in `existing` (3 or more digits, so a >999-clip day
/// keeps counting). Names for other dates are ignored. Returns `1` when none
/// exist for the date.
pub fn next_sequence(date: &str, existing: &[String]) -> u32 {
    let prefix = format!("{date}_");
    existing
        .iter()
        .filter_map(|name| {
            let rest = name.strip_prefix(&prefix)?;
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if digits.len() >= 3 {
                digits.parse::<u32>().ok()
            } else {
                None
            }
        })
        .max()
        .map_or(1, |n| n + 1)
}

/// Format one destination file name. `ext` must already be lowercased and
/// dot-less (e.g. `"mp4"`).
pub fn dest_filename(date: &str, seq: u32, ext: &str) -> String {
    format!("{date}_{seq:03}.{ext}")
}

/// Plan the renames for a batch of sources, assigning consecutive sequence
/// numbers in source order (sources should be pre-sorted by the caller for
/// determinism). `sources` pairs each source path with its lowercased, dot-less
/// extension.
pub fn plan_renames(date: &str, existing: &[String], sources: &[(PathBuf, String)]) -> Vec<Rename> {
    let start = next_sequence(date, existing);
    sources
        .iter()
        .enumerate()
        .map(|(i, (source, ext))| Rename {
            source: source.clone(),
            dest_name: dest_filename(date, start + i as u32, ext),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sequence_for_a_date_is_one() {
        assert_eq!(next_sequence("2026-07-22", &[]), 1);
    }

    #[test]
    fn sequence_continues_from_highest_existing_for_that_date() {
        let existing = vec![
            "2026-07-22_001.mp4".to_string(),
            "2026-07-22_004.mov".to_string(),
            "2026-07-21_009.mp4".to_string(), // other date — ignored
        ];
        assert_eq!(next_sequence("2026-07-22", &existing), 5);
    }

    #[test]
    fn sequence_continues_past_three_digits() {
        let existing = vec!["2026-07-22_999.mp4".to_string()];
        assert_eq!(next_sequence("2026-07-22", &existing), 1000);
        let wide = vec!["2026-07-22_1000.mp4".to_string()];
        assert_eq!(next_sequence("2026-07-22", &wide), 1001);
    }

    #[test]
    fn dest_filename_zero_pads_to_three_digits() {
        assert_eq!(dest_filename("2026-07-22", 7, "mp4"), "2026-07-22_007.mp4");
    }

    #[test]
    fn plan_assigns_consecutive_names_preserving_extension() {
        let existing = vec!["2026-07-22_002.mp4".to_string()];
        let sources = vec![
            (PathBuf::from("CLIP/C0001.MP4"), "mp4".to_string()),
            (PathBuf::from("CLIP/C0002.MOV"), "mov".to_string()),
        ];
        let plan = plan_renames("2026-07-22", &existing, &sources);
        assert_eq!(plan[0].dest_name, "2026-07-22_003.mp4");
        assert_eq!(plan[1].dest_name, "2026-07-22_004.mov");
    }
}
