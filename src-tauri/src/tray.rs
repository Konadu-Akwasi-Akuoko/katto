use tauri::{
    AppHandle, Manager, Wry,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};

use crate::state::AppState;
use crate::window;

/// Tray handles kept alive for the app's lifetime so the menu can be updated
/// without rebuilding the tray.
struct TrayState {
    toggle: MenuItem<Wry>,
    job: MenuItem<Wry>,
    project: MenuItem<Wry>,
    shoot: MenuItem<Wry>,
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "Hide window", true, None::<&str>)?;
    let job = MenuItem::with_id(app, "job", "No active job", false, None::<&str>)?;
    let project = MenuItem::with_id(app, "project", "No project", false, None::<&str>)?;
    let shoot = MenuItem::with_id(app, "shoot", "No shoot scheduled", false, None::<&str>)?;
    let sep_top = PredefinedMenuItem::separator(app)?;
    let sep_bottom = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &toggle,
            &sep_top,
            &job,
            &project,
            &shoot,
            &sep_bottom,
            &quit,
        ],
    )?;

    TrayIconBuilder::with_id("katto")
        .icon(tauri::include_image!("icons/tray/menubar.png"))
        .icon_as_template(true)
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => window::toggle_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    app.manage(TrayState {
        toggle,
        job,
        project,
        shoot,
    });

    Ok(())
}

/// Reflect the main window's presence in the toggle menu item's label.
pub fn set_window_shown(app: &AppHandle, shown: bool) {
    if let Some(state) = app.try_state::<TrayState>() {
        let label = if shown { "Hide window" } else { "Show window" };
        let _ = state.toggle.set_text(label);
    }
}

/// Mirror the active job (label + percent) into the tray; `None` when idle.
/// MenuItem setters marshal to the main thread internally, so this is safe to
/// call from async tasks.
pub fn set_active_job(app: &AppHandle, label: Option<&str>) {
    if let Some(state) = app.try_state::<TrayState>() {
        let _ = state.job.set_text(label.unwrap_or("No active job"));
    }
}

/// The tray's next-shoot line, `"shooting {weekday}: {title}"`. `date` is the
/// shoot's ISO `YYYY-MM-DD`; the weekday is intrinsic to it, so `today` is unused
/// here (kept for a future "today"/"tomorrow" relative wording). An unparseable
/// date degrades to echoing the raw string rather than dropping the line.
fn tray_shoot_line(date: &str, title: &str, _today: &str) -> String {
    match chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") {
        Ok(d) => format!("shooting {}: {}", d.format("%A"), title),
        Err(_) => format!("shooting {date}: {title}"),
    }
}

/// Today's date as an ISO `YYYY-MM-DD` string (UTC), for the next-shoot query.
fn today_iso() -> String {
    use time::OffsetDateTime;
    use time::format_description::well_known::Rfc3339;
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map(|s| s.get(..10).unwrap_or_default().to_string())
        .unwrap_or_default()
}

/// Refresh the two planner lines from the DB: the current-project line from the
/// most-recently-touched project (`—` when none) and the next-shoot line from the
/// earliest upcoming shoot (`No shoot scheduled` when none). Queries off-thread
/// and marshals the `set_text` calls back to the main thread, so it is safe to
/// call from any broadcast/async site.
pub fn refresh_planner_lines(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(db) = app.try_state::<AppState>().map(|s| s.db.clone()) else {
            return;
        };
        let today = today_iso();
        let query_today = today.clone();
        let queried = db
            .call(move |conn| {
                let project = crate::db::projects::most_recently_touched(conn)?;
                let shoot = crate::db::schedule::next_shoot(conn, &query_today)?;
                Ok((project, shoot))
            })
            .await;
        let Ok((project, shoot)) = queried else {
            return;
        };

        let project_text = project.map(|p| p.title).unwrap_or_else(|| "—".to_string());
        let shoot_text = shoot
            .map(|(entry, title)| tray_shoot_line(&entry.date, &title, &today))
            .unwrap_or_else(|| "No shoot scheduled".to_string());

        let _ = app.clone().run_on_main_thread(move || {
            if let Some(state) = app.try_state::<TrayState>() {
                let _ = state.project.set_text(&project_text);
                let _ = state.shoot.set_text(&shoot_text);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_shoot_line_names_the_weekday() {
        // 2026-07-16 is a Thursday.
        assert_eq!(
            tray_shoot_line("2026-07-16", "NVMe deep dive", "2026-07-09"),
            "shooting Thursday: NVMe deep dive"
        );
    }

    #[test]
    fn tray_shoot_line_falls_back_on_unparseable_date() {
        assert_eq!(
            tray_shoot_line("not-a-date", "NVMe deep dive", "2026-07-09"),
            "shooting not-a-date: NVMe deep dive"
        );
    }
}
