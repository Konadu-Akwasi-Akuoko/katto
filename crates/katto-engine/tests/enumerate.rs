use std::path::Path;

use katto_engine::ingest::{CardKind, FileEntry, enumerate::enumerate};

fn walk_files(root: &Path) -> Vec<FileEntry> {
    fn rec(base: &Path, dir: &Path, out: &mut Vec<FileEntry>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                rec(base, &path, out);
            } else {
                let size = entry.metadata().unwrap().len();
                out.push(FileEntry {
                    path: path.strip_prefix(base).unwrap().to_path_buf(),
                    size,
                });
            }
        }
    }
    let mut out = Vec::new();
    rec(root, root, &mut out);
    out
}

#[test]
fn sony_fixture_enumerates_clip_and_sub_groups() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cards/sony");
    let groups = enumerate(CardKind::Sony, &walk_files(&root));
    let labels: Vec<&str> = groups.iter().map(|g| g.label.as_str()).collect();
    assert!(labels.contains(&"CLIP") && labels.contains(&"SUB"));
    let clip = groups.iter().find(|g| g.label == "CLIP").unwrap();
    assert!(
        clip.clips
            .iter()
            .any(|c| c.name == "C0001.MP4" && c.selected)
    );
    assert!(
        clip.clips
            .iter()
            .any(|c| c.name == "C0001M01.XML" && !c.selected)
    );
}
