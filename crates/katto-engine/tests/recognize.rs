use std::path::{Path, PathBuf};

use katto_engine::ingest::{CardKind, VolumeTree, recognize::recognize};

/// Walk a fixture directory into a `VolumeTree` of paths relative to `root` —
/// the same in-memory shape the app crate's real walk produces.
fn walk(root: &Path) -> VolumeTree {
    fn rec(base: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            out.push(path.strip_prefix(base).unwrap().to_path_buf());
            if path.is_dir() {
                rec(base, &path, out);
            }
        }
    }
    let mut entries = Vec::new();
    rec(root, root, &mut entries);
    VolumeTree { entries }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/cards")
        .join(name)
}

#[test]
fn sony_fixture_tree_is_a_sony_card() {
    let card = recognize(&walk(&fixture("sony"))).unwrap();
    assert_eq!(card.kind, CardKind::Sony);
    assert!(
        card.clip_roots
            .contains(&PathBuf::from("PRIVATE/M4ROOT/CLIP"))
    );
}

#[test]
fn iphone_fixture_tree_is_an_iphone_card() {
    assert_eq!(
        recognize(&walk(&fixture("iphone-dcim"))).unwrap().kind,
        CardKind::IphoneDcim
    );
}

#[test]
fn not_a_card_fixture_is_none() {
    assert!(recognize(&walk(&fixture("not-a-card"))).is_none());
}
