//! Exact rational time: every timestamp/duration in the engine is a ratio in
//! the media's timebase, never a float.

use serde::{Deserialize, Serialize};

/// An exact `num/den` ratio (e.g. a `30000/1001` frame rate or a timestamp in
/// a media timebase). Floats appear only at UI and model boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rational {
    /// Numerator (signed: timestamps can be negative in some containers).
    pub num: i64,
    /// Denominator; never zero.
    pub den: u32,
}
