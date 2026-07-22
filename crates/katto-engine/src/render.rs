//! Kept-only MP4 render: keep-window math, deterministic filtergraph text,
//! and the thin ffmpeg spawn site (Task 2).

pub mod segments;

pub use segments::{Keep, coalesce_cuts, filter_complex_script, keep_windows};
