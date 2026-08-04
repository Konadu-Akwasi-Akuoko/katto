use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::db;
use crate::error::{Error, Result};
use crate::state::AppState;

/// One dot on the calendar. `date` is always `YYYY-MM-DD` (the day the marker
/// lands on); historical markers derive it from the event/idea timestamp.
#[derive(Debug, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CalendarMarker {
    Shoot {
        project_slug: String,
        title: String,
        date: String,
        note: Option<String>,
    },
    Publish {
        project_slug: String,
        title: String,
        date: String,
        note: Option<String>,
    },
    Backlog {
        idea_id: String,
        title: String,
        date: String,
    },
    Phase {
        project_slug: String,
        title: String,
        date: String,
        to: String,
    },
}

/// The day after `to` (`YYYY-MM-DD`), the exclusive upper bound used for the
/// datetime columns (`events.ts`, `ideas.first_seen`) so their indexes are used.
fn next_day(to: &str) -> Result<String> {
    let mut parts = to.split('-');
    let y: Option<i32> = parts.next().and_then(|v| v.parse().ok());
    let m: Option<u8> = parts.next().and_then(|v| v.parse().ok());
    let d: Option<u8> = parts.next().and_then(|v| v.parse().ok());
    let (Some(y), Some(m), Some(d)) = (y, m, d) else {
        return Err(Error::Io(format!("bad calendar bound: {to}")));
    };
    let month = time::Month::try_from(m).map_err(|e| Error::Io(e.to_string()))?;
    let date = time::Date::from_calendar_date(y, month, d).map_err(|e| Error::Io(e.to_string()))?;
    let next = date
        .next_day()
        .ok_or_else(|| Error::Io("calendar bound overflow".to_string()))?;
    Ok(format!(
        "{:04}-{:02}-{:02}",
        next.year(),
        next.month() as u8,
        next.day()
    ))
}

/// The `YYYY-MM-DD` prefix of a datetime string. Falls back to the whole string
/// when it is shorter than 10 chars or not char-aligned at 10 — externally
/// imported `first_seen`/`ts` values are not trusted to be well-formed.
fn date_part(s: &str) -> String {
    s.get(..10).unwrap_or(s).to_string()
}

/// The connection-level body of [`list_calendar`], split out so it is unit-testable
/// against an in-memory database without the Tauri state wrapper.
fn list_calendar_inner(conn: &Connection, from: &str, to: &str) -> Result<Vec<CalendarMarker>> {
    let to_excl = next_day(to)?;
    let mut out = Vec::new();

    for e in db::schedule::list_range(conn, from, to)? {
        let title = db::projects::get(conn, &e.project_slug)?
            .map(|p| p.title)
            .unwrap_or_else(|| e.project_slug.clone());
        out.push(match e.kind.as_str() {
            "publish" => CalendarMarker::Publish {
                project_slug: e.project_slug,
                title,
                date: e.date,
                note: e.note,
            },
            _ => CalendarMarker::Shoot {
                project_slug: e.project_slug,
                title,
                date: e.date,
                note: e.note,
            },
        });
    }

    for (id, title, first_seen) in db::ideas::list_added_between(conn, from, &to_excl)? {
        out.push(CalendarMarker::Backlog {
            idea_id: id,
            title,
            date: date_part(&first_seen),
        });
    }

    for ev in db::events::list_range_by_kind(conn, "project-status-changed", from, &to_excl)? {
        let Some(slug) = ev.project_slug else {
            continue;
        };
        let Some(payload) = ev.payload_json.as_deref() else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(payload) else {
            continue;
        };
        let Some(to_phase) = parsed.get("to").and_then(|v| v.as_str()) else {
            continue;
        };
        let title = db::projects::get(conn, &slug)?
            .map(|p| p.title)
            .unwrap_or_else(|| slug.clone());
        out.push(CalendarMarker::Phase {
            project_slug: slug,
            title,
            date: date_part(&ev.ts),
            to: to_phase.to_string(),
        });
    }

    Ok(out)
}

/// All calendar markers whose day falls in `[from, to]` (inclusive ISO dates):
/// shoot/publish pins from the `schedule` table, backlog-added from
/// `ideas.first_seen`, and phase moves from `project-status-changed` events (the
/// destination phase). One round-trip; the frontend does the category/project
/// filtering client-side.
#[tauri::command]
#[specta::specta]
pub async fn list_calendar(
    state: State<'_, AppState>,
    from: String,
    to: String,
) -> Result<Vec<CalendarMarker>> {
    state
        .db
        .call(move |conn| list_calendar_inner(conn, &from, &to))
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::projects::{self, Project};
    use crate::db::test_db;
    use crate::db::{events, ideas, schedule};

    #[test]
    fn next_day_rolls_over_month_and_year() {
        assert_eq!(next_day("2026-07-31").unwrap(), "2026-08-01");
        assert_eq!(next_day("2026-12-31").unwrap(), "2027-01-01");
        assert_eq!(next_day("2026-07-10").unwrap(), "2026-07-11");
    }

    #[test]
    fn next_day_rejects_a_bad_bound() {
        assert!(next_day("nope").is_err());
    }

    #[test]
    fn date_part_takes_the_iso_day_and_falls_back_on_short_input() {
        assert_eq!(date_part("2026-07-20T13:45:00Z"), "2026-07-20");
        assert_eq!(date_part("2026-07-20"), "2026-07-20");
        assert_eq!(date_part("bad"), "bad");
    }

    fn seed_project(conn: &rusqlite::Connection, slug: &str) {
        projects::insert(
            conn,
            &Project {
                slug: slug.to_string(),
                title: "NVMe Deep Dive".to_string(),
                root_path: format!("/Volumes/Studio/Projects/{slug}"),
                status: "editing".to_string(),
                target_nle: "fcp".to_string(),
                priority: "none".to_string(),
                shoot_date: None,
                publish_date: None,
                created_at: "2026-07-09T00:00:00Z".to_string(),
                last_touched_at: None,
                kind: "unset".to_string(),
            },
        )
        .unwrap();
    }

    fn seed_backlog_idea(conn: &rusqlite::Connection, id: &str, first_seen: &str) {
        ideas::upsert_imported(
            conn,
            &ideas::Idea {
                id: id.to_string(),
                r#type: "manual".to_string(),
                kind: "unset".to_string(),
                status: "backlog".to_string(),
                title: "Fresh Idea".to_string(),
                rationale: None,
                source: None,
                source_url: None,
                source_title: None,
                evidence_json: None,
                raw_signal_id: None,
                first_seen: first_seen.to_string(),
                notes: None,
                promoted_slug: None,
                kind_source: None,
                kind_why: None,
            },
        )
        .unwrap();
    }

    #[test]
    fn list_calendar_inner_unions_pins_backlog_and_phase() {
        let conn = test_db();
        seed_project(&conn, "p-2026-07-09");
        schedule::upsert(&conn, "p-2026-07-09", "shoot", "2026-08-01", None).unwrap();
        seed_backlog_idea(&conn, "idea-1", "2026-07-20T08:00:00Z");
        events::record(
            &conn,
            "project-status-changed",
            Some("p-2026-07-09"),
            Some(r#"{"from":"idea","to":"editing"}"#),
        )
        .unwrap();

        let markers = list_calendar_inner(&conn, "2000-01-01", "2099-12-31").unwrap();

        assert!(
            markers
                .iter()
                .any(|m| matches!(m, CalendarMarker::Shoot { date, .. } if date == "2026-08-01")),
            "expected a shoot pin marker"
        );
        assert!(
            markers
                .iter()
                .any(|m| matches!(m, CalendarMarker::Backlog { date, .. } if date == "2026-07-20")),
            "expected a backlog marker dated by first_seen"
        );
        assert!(
            markers
                .iter()
                .any(|m| matches!(m, CalendarMarker::Phase { to, .. } if to == "editing")),
            "expected a phase marker for the destination status"
        );
    }
}
