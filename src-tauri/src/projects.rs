pub mod manifest;
pub mod slug;

pub use manifest::{
    MANIFEST_FILE, MANIFEST_SCHEMA_VERSION, ProjectManifest, read_manifest, write_manifest,
};
pub use slug::{kebab_slug, project_slug};
