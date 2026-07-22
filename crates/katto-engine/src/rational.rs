//! Exact rational time: every timestamp/duration in the engine is a ratio in
//! the media's timebase, never a float.

use serde::{Deserialize, Serialize};

/// An exact `num/den` ratio (e.g. a `30000/1001` frame rate or a timestamp in
/// a media timebase). Floats appear only at UI and model boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Rational {
    /// Numerator (signed: timestamps can be negative in some containers).
    /// Exported to TS as `number`: real values are tick counts well inside the
    /// safe-integer range.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub num: i64,
    /// Denominator; never zero.
    pub den: u32,
}

impl Rational {
    /// Construct a rational; `den` must be non-zero (all call sites pass fixed timebases).
    pub const fn new(num: i64, den: u32) -> Self {
        Self { num, den }
    }

    /// Decimal seconds -> nearest tick of `timebase` (ticks per second).
    /// Non-finite input maps to zero ticks; upstream validation rejects it earlier.
    pub fn from_seconds(secs: f64, timebase: u32) -> Self {
        let ticks = secs * f64::from(timebase);
        let num = if ticks.is_finite() {
            ticks.round() as i64
        } else {
            0
        };
        Self { num, den: timebase }
    }

    /// Projection to display/model seconds. Boundary use only.
    pub fn to_secs_f64(self) -> f64 {
        self.num as f64 / f64::from(self.den)
    }

    /// Convert to a new timebase, rounding to the nearest tick (exact integer math).
    pub fn rescale(self, den: u32) -> Self {
        let num = div_round_nearest(i128::from(self.num) * i128::from(den), i128::from(self.den));
        Self {
            num: num as i64,
            den,
        }
    }

    /// `self + rhs`; same-den fast path preserves the den, mixed dens use the lcm.
    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        combine(self, rhs, i128::checked_add)
    }

    /// `self - rhs`; same rules as [`Rational::checked_add`].
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        combine(self, rhs, i128::checked_sub)
    }

    /// Scale the numerator by an integer factor; den preserved.
    pub fn checked_mul_int(self, k: i64) -> Option<Self> {
        let num = self.num.checked_mul(k)?;
        Some(Self { num, den: self.den })
    }

    /// Snap to the nearest integer frame of `fps`, returned in `self`'s timebase.
    pub fn snap_to_frame(self, fps: Rational) -> Self {
        let frame = div_round_nearest(
            i128::from(self.num) * i128::from(fps.num),
            i128::from(self.den) * i128::from(fps.den),
        );
        // frame * fps.den / fps.num seconds, back in self.den ticks:
        let num = div_round_nearest(
            frame * i128::from(fps.den) * i128::from(self.den),
            i128::from(fps.num),
        );
        Self {
            num: num as i64,
            den: self.den,
        }
    }
}

impl PartialOrd for Rational {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Rational {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let lhs = i128::from(self.num) * i128::from(other.den);
        let rhs = i128::from(other.num) * i128::from(self.den);
        lhs.cmp(&rhs)
    }
}

/// Round-half-away-from-zero integer division.
fn div_round_nearest(num: i128, den: i128) -> i128 {
    debug_assert!(den > 0);
    if num >= 0 {
        (num + den / 2) / den
    } else {
        (num - den / 2) / den
    }
}

fn combine(a: Rational, b: Rational, op: fn(i128, i128) -> Option<i128>) -> Option<Rational> {
    if a.den == b.den {
        let num = op(i128::from(a.num), i128::from(b.num))?;
        return Some(Rational {
            num: i64::try_from(num).ok()?,
            den: a.den,
        });
    }
    let g = gcd(u64::from(a.den), u64::from(b.den));
    let lcm = u64::from(a.den) / g * u64::from(b.den);
    let den = u32::try_from(lcm).ok()?;
    let an = i128::from(a.num) * i128::from(lcm / u64::from(a.den));
    let bn = i128::from(b.num) * i128::from(lcm / u64::from(b.den));
    let num = op(an, bn)?;
    Some(Rational {
        num: i64::try_from(num).ok()?,
        den,
    })
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const NTSC: Rational = Rational {
        num: 30000,
        den: 1001,
    };

    #[test]
    fn from_seconds_rounds_to_nearest_tick() {
        assert_eq!(
            Rational::from_seconds(1.0, 30000),
            Rational::new(30000, 30000)
        );
        assert_eq!(Rational::from_seconds(0.5, 1000), Rational::new(500, 1000));
        assert_eq!(
            Rational::from_seconds(4.21, 1000),
            Rational::new(4210, 1000)
        );
    }

    #[test]
    fn add_same_den_preserves_den() {
        let a = Rational::new(1001, 30000);
        let b = Rational::new(2002, 30000);
        assert_eq!(a.checked_add(b), Some(Rational::new(3003, 30000)));
    }

    #[test]
    fn add_mixed_den_uses_lcm() {
        let a = Rational::new(1, 2);
        let b = Rational::new(1, 3);
        assert_eq!(a.checked_add(b), Some(Rational::new(5, 6)));
    }

    #[test]
    fn sub_can_go_negative() {
        let a = Rational::new(1, 10);
        let b = Rational::new(3, 10);
        assert_eq!(a.checked_sub(b), Some(Rational::new(-2, 10)));
    }

    #[test]
    fn ordering_is_cross_denominator() {
        assert!(Rational::new(1, 3) < Rational::new(1, 2));
        assert_eq!(
            Rational::new(30000, 30000).cmp(&Rational::new(1000, 1000)),
            std::cmp::Ordering::Equal
        );
        assert!(Rational::new(-1, 2) < Rational::new(0, 5));
    }

    #[test]
    fn rescale_rounds_to_nearest() {
        // 1/3 s at den 1000 = 333.33 ticks -> 333
        assert_eq!(Rational::new(1, 3).rescale(1000), Rational::new(333, 1000));
        // 2/3 s -> 666.67 -> 667
        assert_eq!(Rational::new(2, 3).rescale(1000), Rational::new(667, 1000));
    }

    #[test]
    fn snap_to_frame_ntsc() {
        // 0.5s at 30000/1001 fps: 0.5*30000/1001 = 14.985 frames -> frame 15
        // -> 15*1001/30000 s = 0.5005 s; in den 1000 that's 500.5 -> 501 ticks
        let t = Rational::new(500, 1000);
        assert_eq!(t.snap_to_frame(NTSC), Rational::new(501, 1000));
    }

    #[test]
    fn checked_mul_int_scales_numerator_only() {
        assert_eq!(
            Rational::new(1001, 30000).checked_mul_int(3),
            Some(Rational::new(3003, 30000))
        );
        assert_eq!(Rational::new(i64::MAX, 1000).checked_mul_int(2), None);
    }

    proptest! {
        #[test]
        fn from_seconds_to_secs_round_trip(secs in 0.0f64..36_000.0, tb in prop::sample::select(vec![1000u32, 30000, 24000, 60000, 90000])) {
            let r = Rational::from_seconds(secs, tb);
            let back = r.to_secs_f64();
            // within half a tick
            prop_assert!((back - secs).abs() <= 0.5 / tb as f64 + 1e-9);
        }

        #[test]
        fn add_then_sub_is_identity(n1 in -1_000_000i64..1_000_000, n2 in -1_000_000i64..1_000_000, d in prop::sample::select(vec![1000u32, 30000, 1001])) {
            let a = Rational::new(n1, d);
            let b = Rational::new(n2, d);
            let sum = a.checked_add(b).unwrap();
            prop_assert_eq!(sum.checked_sub(b).unwrap(), a);
            prop_assert_eq!(sum.den, d); // den preserved
        }
    }
}
