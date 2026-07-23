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
        .call(move |conn| {
            let to_excl = next_day(&to)?;
            let mut out = Vec::new();

            for e in db::schedule::list_range(conn, &from, &to)? {
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

            for (id, title, first_seen) in db::ideas::list_added_between(conn, &from, &to_excl)? {
                out.push(CalendarMarker::Backlog {
                    idea_id: id,
                    title,
                    date: first_seen[..10].to_string(),
                });
            }

            for ev in
                db::events::list_range_by_kind(conn, "project-status-changed", &from, &to_excl)?
            {
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
                    date: ev.ts[..10].to_string(),
                    to: to_phase.to_string(),
                });
            }

            Ok(out)
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
