use rusqlite::Connection;

use crate::error::Result;

/// Count of rows judged at/after `iso_utc`, partitioned as (kept, discarded).
pub fn judged_counts_since(conn: &Connection, iso_utc: &str) -> Result<(u32, u32)> {
    let mut stmt = conn.prepare(
        "SELECT
           COALESCE(SUM(judged_verdict = 'kept'), 0),
           COALESCE(SUM(judged_verdict = 'discarded'), 0)
         FROM raw_signal WHERE judged_at >= ?1",
    )?;
    let counts = stmt.query_row([iso_utc], |row| {
        Ok((row.get::<_, u32>(0)?, row.get::<_, u32>(1)?))
    })?;
    Ok(counts)
}

/// Rows not yet judged by any curation pass.
pub fn unjudged_count(conn: &Connection) -> Result<u32> {
    let n = conn.query_row(
        "SELECT COUNT(*) FROM raw_signal WHERE judged_at IS NULL",
        [],
        |r| r.get::<_, u32>(0),
    )?;
    Ok(n)
}

/// Housekeeping (PRD): rows judged more than `days` ago are deleted; unjudged
/// rows never. The retention clock starts at judging — a signal fetched long
/// ago but only just judged keeps its full audit window.
pub fn prune_judged_older_than_days(conn: &Connection, days: u32) -> Result<usize> {
    let n = conn.execute(
        "DELETE FROM raw_signal
         WHERE judged_at IS NOT NULL AND judged_at < datetime('now', '-' || ?1 || ' days')",
        [days],
    )?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_db;

    fn seed_row(
        conn: &rusqlite::Connection,
        id: &str,
        judged_at: Option<&str>,
        verdict: Option<&str>,
        fetched_at: &str,
    ) {
        conn.execute(
            "INSERT INTO raw_signal (id, source, external_id, title, payload_json, fetched_at, judged_at, judged_verdict)
             VALUES (?1, 'hn', ?1, 'T', '{}', ?2, ?3, ?4)",
            rusqlite::params![id, fetched_at, judged_at, verdict],
        )
        .unwrap();
    }

    #[test]
    fn judged_counts_since_partitions_by_verdict() {
        let conn = test_db();
        seed_row(
            &conn,
            "a",
            Some("2026-07-22 08:01:00"),
            Some("kept"),
            "2026-07-20 00:00:00",
        );
        seed_row(
            &conn,
            "b",
            Some("2026-07-22 08:02:00"),
            Some("discarded"),
            "2026-07-20 00:00:00",
        );
        seed_row(
            &conn,
            "c",
            Some("2026-07-21 00:00:00"),
            Some("discarded"),
            "2026-07-20 00:00:00",
        );
        seed_row(&conn, "d", None, None, "2026-07-20 00:00:00");
        let (kept, discarded) = judged_counts_since(&conn, "2026-07-22 08:00:00").unwrap();
        assert_eq!((kept, discarded), (1, 1));
    }

    #[test]
    fn unjudged_count_counts_null_judged_at() {
        let conn = test_db();
        seed_row(&conn, "a", None, None, "2026-07-20 00:00:00");
        seed_row(
            &conn,
            "b",
            Some("2026-07-21 00:00:00"),
            Some("kept"),
            "2026-07-20 00:00:00",
        );
        assert_eq!(unjudged_count(&conn).unwrap(), 1);
    }

    #[test]
    fn prune_deletes_only_old_judged_rows() {
        let conn = test_db();
        seed_row(
            &conn,
            "old-judged",
            Some("2026-01-01 00:00:00"),
            Some("discarded"),
            "2026-01-01 00:00:00",
        );
        seed_row(&conn, "old-unjudged", None, None, "2026-01-01 00:00:00");
        seed_row(
            &conn,
            "new-judged",
            Some("2026-07-21 00:00:00"),
            Some("kept"),
            "2026-07-21 00:00:00",
        );
        let n = prune_judged_older_than_days(&conn, 90).unwrap();
        assert_eq!(n, 1);
        assert_eq!(unjudged_count(&conn).unwrap(), 1);
    }

    #[test]
    fn prune_clock_starts_at_judging_not_fetching() {
        let conn = test_db();
        // Fetched long ago but only judged recently: the retention window is
        // "90 days after judging", so this row must survive.
        seed_row(
            &conn,
            "old-fetch-new-judge",
            Some("2026-07-21 00:00:00"),
            Some("kept"),
            "2026-01-01 00:00:00",
        );
        let n = prune_judged_older_than_days(&conn, 90).unwrap();
        assert_eq!(n, 0, "recently judged rows survive regardless of fetch age");
    }
}
