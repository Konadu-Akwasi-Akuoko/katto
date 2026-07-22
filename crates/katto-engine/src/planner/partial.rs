//! Incremental cuts extractor: pull complete `Cut` objects out of a *prefix*
//! of planner output so the UI can show cuts while the model is still
//! emitting. Pure and never-erroring.

use crate::planner::balanced_object;
use crate::schema::Cut;

/// Extract the complete `Cut` objects from a prefix of planner output text.
/// Incomplete tails yield what is parseable so far; never errors.
pub fn cuts_prefix(text: &str) -> Vec<Cut> {
    let Some(key) = text.find("\"cuts\"") else {
        return Vec::new();
    };
    let after_key = &text[key + "\"cuts\"".len()..];
    let Some(bracket) = after_key.find('[') else {
        return Vec::new();
    };
    let mut rest = &after_key[bracket + 1..];
    let mut cuts = Vec::new();
    loop {
        let Some(obj_start) = rest.find(|c: char| !c.is_whitespace() && c != ',') else {
            break;
        };
        if !rest[obj_start..].starts_with('{') {
            break; // ']' ends the array, anything else is an unparseable tail
        }
        let Some(slice) = balanced_object(&rest[obj_start..]) else {
            break; // incomplete trailing object
        };
        match serde_json::from_str::<Cut>(slice) {
            Ok(cut) => cuts.push(cut),
            Err(_) => break,
        }
        rest = &rest[obj_start + slice.len()..];
    }
    cuts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_prefix_yields_nothing() {
        assert!(cuts_prefix("").is_empty());
        assert!(cuts_prefix("{\"source_duration_secs\": 12.0, \"cuts\": [").is_empty());
    }

    #[test]
    fn complete_objects_in_incomplete_array_are_extracted() {
        let prefix = r#"{"source_duration_secs": 12.0, "cuts": [
            {"start": 1.0, "end": 1.5, "reason": "filler", "excerpt": "um"},
            {"start": 3.0, "end": 3.5, "reason": "stutter", "excerpt": "I-I"},
            {"start": 5.0, "end"#;
        let cuts = cuts_prefix(prefix);
        assert_eq!(cuts.len(), 2);
        assert_eq!(cuts[1].excerpt, "I-I");
    }

    #[test]
    fn full_document_extracts_all_cuts() {
        let raw = std::fs::read_to_string(format!(
            "{}/tests/fixtures/cuts.valid.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        assert_eq!(cuts_prefix(&raw).len(), 2);
    }
}
