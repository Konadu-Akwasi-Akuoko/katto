//! Post-copy verification: file-count parity and per-file byte-size match.

use std::collections::HashMap;

use crate::ingest::VerifyError;

/// Compare the expected copy set against what landed on disk. Returns an empty
/// vec when every file is present at its exact source size; otherwise a list of
/// every discrepancy (count mismatch first, then per-file size/missing errors in
/// `expected` order). Inputs are `(dest_name, byte_size)` pairs.
pub fn verify(expected: &[(String, u64)], actual: &[(String, u64)]) -> Vec<VerifyError> {
    let mut errors = Vec::new();
    if expected.len() != actual.len() {
        errors.push(VerifyError::CountMismatch {
            expected: expected.len(),
            actual: actual.len(),
        });
    }
    let actual_by_name: HashMap<&str, u64> = actual.iter().map(|(n, s)| (n.as_str(), *s)).collect();
    for (name, expected_size) in expected {
        match actual_by_name.get(name.as_str()) {
            None => errors.push(VerifyError::Missing { name: name.clone() }),
            Some(&actual_size) if actual_size != *expected_size => {
                errors.push(VerifyError::SizeMismatch {
                    name: name.clone(),
                    expected: *expected_size,
                    actual: actual_size,
                });
            }
            Some(_) => {}
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(name: &str, size: u64) -> (String, u64) {
        (name.to_string(), size)
    }

    #[test]
    fn identical_sets_have_no_errors() {
        let e = vec![pair("a.mp4", 100), pair("b.mov", 200)];
        assert!(verify(&e, &e).is_empty());
    }

    #[test]
    fn size_mismatch_is_reported() {
        let e = vec![pair("a.mp4", 100)];
        let a = vec![pair("a.mp4", 99)];
        assert_eq!(
            verify(&e, &a),
            vec![VerifyError::SizeMismatch {
                name: "a.mp4".to_string(),
                expected: 100,
                actual: 99
            }]
        );
    }

    #[test]
    fn missing_file_reports_count_and_missing() {
        let e = vec![pair("a.mp4", 100), pair("b.mov", 200)];
        let a = vec![pair("a.mp4", 100)];
        let errors = verify(&e, &a);
        assert!(errors.contains(&VerifyError::CountMismatch {
            expected: 2,
            actual: 1
        }));
        assert!(errors.contains(&VerifyError::Missing {
            name: "b.mov".to_string()
        }));
    }
}
