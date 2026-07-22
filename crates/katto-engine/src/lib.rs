#![warn(missing_docs)]
//! Pure media-pipeline library: rational time, cut schemas, ffprobe parsing,
//! and SD-ingest logic. Never depends on tauri or any UI concern.

pub mod error;
pub mod ffprobe;
pub mod ingest;
pub mod merge;
pub mod rational;
pub mod schema;
pub mod validate;

pub use error::{Error, Result};
pub use rational::Rational;
