#![warn(missing_docs)]
//! Pure media-pipeline library: rational time, cut schemas, ffprobe parsing,
//! and SD-ingest logic. Never depends on tauri or any UI concern.

pub mod error;
pub mod rational;
pub mod schema;

pub use error::{Error, Result};
pub use rational::Rational;
