use rusqlite::Connection;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

use crate::db::{events, settings as settings_repo};
use crate::error::{Error, Result};
use crate::keychain::{self, KeyService};
use crate::paths::{self, RootCheck};
use crate::state::AppState;

/// Open the native folder picker and inspect the chosen directory. `None` when
/// the user cancels. Nothing is persisted here — the wizard saves the path via
/// `set_settings` when the step is confirmed.
#[tauri::command]
#[specta::specta]
pub async fn pick_studio_root(app: tauri::AppHandle) -> Result<Option<RootCheck>> {
    let picked =
        tauri::async_runtime::spawn_blocking(move || app.dialog().file().blocking_pick_folder())
            .await
            .map_err(|e| Error::Io(e.to_string()))?;

    let Some(file_path) = picked else {
        return Ok(None);
    };
    let path = file_path
        .into_path()
        .map_err(|e| Error::Io(e.to_string()))?;
    Ok(Some(paths::check_root(&path)))
}

/// Store a credential in the macOS keychain. The value is write-only: never
/// echoed back, never logged. Presence is recorded in the activity log by
/// service name only.
#[tauri::command]
#[specta::specta]
pub async fn store_key(
    state: State<'_, AppState>,
    service: KeyService,
    value: String,
) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || keychain::store_key(service, &value))
        .await
        .map_err(|e| Error::Keychain(e.to_string()))??;

    state
        .db
        .call(move |conn| {
            events::record(
                conn,
                "key_stored",
                None,
                Some(&format!("{{\"service\":\"{}\"}}", service.account())),
            )
        })
        .await?;
    Ok(())
}

/// Whether a credential exists for `service`, without exposing its value.
#[tauri::command]
#[specta::specta]
pub async fn key_present(service: KeyService) -> Result<bool> {
    tauri::async_runtime::spawn_blocking(move || keychain::key_present(service))
        .await
        .map_err(|e| Error::Keychain(e.to_string()))?
}

/// `which claude` through a login shell, so the owner's PATH additions apply.
/// A found path is cached in settings for later phases; not-found is a normal
/// outcome, not an error.
#[tauri::command]
#[specta::specta]
pub async fn detect_claude(state: State<'_, AppState>) -> Result<Option<String>> {
    let output = tauri::async_runtime::spawn_blocking(|| {
        std::process::Command::new("zsh")
            .args(["-lc", "which claude"])
            .output()
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))??;

    let found = parse_which_output(
        output.status.success(),
        &String::from_utf8_lossy(&output.stdout),
    );
    if let Some(path) = found.clone() {
        state
            .db
            .call(move |conn| settings_repo::set(conn, "claude_path", &path))
            .await?;
    }
    Ok(found)
}

/// Finish the wizard: a saved studio root is the one hard requirement; keys
/// stay optional (Phase 1 runs no AI).
#[tauri::command]
#[specta::specta]
pub async fn complete_onboarding(state: State<'_, AppState>) -> Result<()> {
    state.db.call(|conn| complete(conn)).await
}

fn complete(conn: &Connection) -> Result<()> {
    if settings_repo::get(conn, "studio_root")?.is_none() {
        return Err(Error::Onboarding(
            "pick a studio root before finishing onboarding".to_string(),
        ));
    }
    settings_repo::set(conn, "onboarding_complete", "true")?;
    events::record(conn, "onboarding_completed", None, None)?;
    Ok(())
}

/// Extract the binary path from `which` output: success status plus a single
/// absolute-path line, else `None`.
fn parse_which_output(success: bool, stdout: &str) -> Option<String> {
    if !success {
        return None;
    }
    let line = stdout.lines().next()?.trim();
    line.starts_with('/').then(|| line.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_db;

    #[test]
    fn complete_without_root_is_rejected() {
        let conn = test_db();
        let err = complete(&conn).unwrap_err();
        assert!(matches!(err, crate::error::Error::Onboarding(_)));
    }

    #[test]
    fn complete_sets_flag_and_records_event() {
        let conn = test_db();
        crate::db::settings::set(&conn, "studio_root", "/Volumes/Studio").unwrap();
        complete(&conn).unwrap();

        assert_eq!(
            crate::db::settings::get(&conn, "onboarding_complete")
                .unwrap()
                .as_deref(),
            Some("true")
        );
        let events = crate::db::events::list(&conn, 1, None).unwrap();
        assert_eq!(events[0].kind, "onboarding_completed");
    }

    #[test]
    fn which_output_parses_only_successful_absolute_paths() {
        assert_eq!(
            parse_which_output(true, "/opt/homebrew/bin/claude\n"),
            Some("/opt/homebrew/bin/claude".to_string())
        );
        assert_eq!(parse_which_output(false, "claude not found\n"), None);
        assert_eq!(parse_which_output(true, ""), None);
        assert_eq!(parse_which_output(true, "claude not found"), None);
    }
}
