use chrono::{Duration, NaiveDateTime};

/// A parsed `daily@HH:MM;catchup=<N>h` schedule. All due-math is naive LOCAL
/// time — the runtime feeds `Local::now().naive_local()`.
#[derive(Debug, PartialEq, Clone)]
pub struct ScheduleSpec {
    pub hour: u32,
    pub minute: u32,
    pub catchup: Duration,
}

/// Parse the spec grammar `daily@HH:MM;catchup=<N>h`. Both parts are required;
/// anything else is `None` (the runtime flags bad specs, never guesses).
pub fn parse_spec(spec: &str) -> Option<ScheduleSpec> {
    let (time_part, catchup_part) = spec.split_once(';')?;
    let hhmm = time_part.strip_prefix("daily@")?;
    let (hh, mm) = hhmm.split_once(':')?;
    let hour: u32 = hh.parse().ok()?;
    let minute: u32 = mm.parse().ok()?;
    if hour >= 24 || minute >= 60 {
        return None;
    }
    let hours_str = catchup_part.strip_prefix("catchup=")?.strip_suffix('h')?;
    let catchup_hours: i64 = hours_str.parse().ok()?;
    if catchup_hours <= 0 {
        return None;
    }
    Some(ScheduleSpec {
        hour,
        minute,
        catchup: Duration::hours(catchup_hours),
    })
}

/// Level-triggered due check: today's slot has passed AND the last success is
/// absent or older than the catch-up window. The runtime writing
/// `last_success_at` on completion is what turns any number of missed days
/// into exactly one catch-up run.
pub fn is_due(
    spec: &ScheduleSpec,
    last_success: Option<NaiveDateTime>,
    now: NaiveDateTime,
) -> bool {
    let Some(slot) = now.date().and_hms_opt(spec.hour, spec.minute, 0) else {
        return false;
    };
    if now < slot {
        return false;
    }
    match last_success {
        None => true,
        Some(last) => now - last >= spec.catchup,
    }
}

/// Exponential retry backoff for failed runs: 1, 2, 4, … minutes, capped at 60.
pub fn retry_backoff(consecutive_failures: u32) -> Duration {
    let exponent = consecutive_failures.saturating_sub(1).min(6);
    Duration::minutes((1_i64 << exponent).min(60))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, NaiveDate, NaiveDateTime};

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, mo, d)
            .unwrap()
            .and_hms_opt(h, mi, 0)
            .unwrap()
    }

    fn nightly() -> ScheduleSpec {
        parse_spec("daily@00:00;catchup=20h").unwrap()
    }

    #[test]
    fn parse_round_trips_fields() {
        let s = parse_spec("daily@02:30;catchup=20h").unwrap();
        assert_eq!((s.hour, s.minute), (2, 30));
        assert_eq!(s.catchup, Duration::hours(20));
    }

    #[test]
    fn parse_rejects_malformed() {
        for bad in [
            "daily@25:00;catchup=20h",
            "daily@00:00",
            "weekly@00:00;catchup=20h",
            "",
            "daily@0:0;catchup=h",
        ] {
            assert!(parse_spec(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn never_run_and_slot_passed_is_due() {
        assert!(is_due(&nightly(), None, at(2026, 7, 22, 0, 1)));
    }

    #[test]
    fn before_todays_slot_not_due() {
        let s = parse_spec("daily@23:00;catchup=20h").unwrap();
        assert!(!is_due(&s, None, at(2026, 7, 22, 8, 0)));
    }

    #[test]
    fn slept_through_slot_runs_once_on_wake() {
        // succeeded yesterday 00:05, Mac slept through 00:00, wakes 08:00 → 32h ago → due
        assert!(is_due(
            &nightly(),
            Some(at(2026, 7, 21, 0, 5)),
            at(2026, 7, 22, 8, 0)
        ));
    }

    #[test]
    fn already_ran_today_not_due() {
        // ran 00:05 today, now 09:00 → 9h < 20h catch-up → quiet
        assert!(!is_due(
            &nightly(),
            Some(at(2026, 7, 22, 0, 5)),
            at(2026, 7, 22, 9, 0)
        ));
    }

    #[test]
    fn multiple_missed_days_still_one_run() {
        // is_due is level-triggered; the runtime writes last_success on completion,
        // so a 3-day gap yields exactly one run — due before, not-due after success.
        let before = is_due(
            &nightly(),
            Some(at(2026, 7, 18, 0, 5)),
            at(2026, 7, 22, 10, 0),
        );
        let after = is_due(
            &nightly(),
            Some(at(2026, 7, 22, 10, 5)),
            at(2026, 7, 22, 10, 6),
        );
        assert!(before);
        assert!(!after);
    }

    #[test]
    fn backoff_doubles_and_caps() {
        assert_eq!(retry_backoff(1), Duration::minutes(1));
        assert_eq!(retry_backoff(3), Duration::minutes(4));
        assert_eq!(retry_backoff(10), Duration::minutes(60));
    }
}
