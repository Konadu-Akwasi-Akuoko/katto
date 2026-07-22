//! FCPXML 1.11 emitter. The typed builder lives in [`builder`]; the emitter
//! body (bundle -> document -> validated XML) lands with the emitter task.

pub mod builder;

pub use builder::{FcpxmlDoc, RescueClip, SpineClip, time_attr, write_document};
