use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

use crate::db::settings as repo;
use crate::error::Error;
use crate::error::Result;
use crate::keychain::{self, KeyService};
use crate::state::AppState;

/// Minutes of inactivity before a dock session is reaped (D15: default 5,
/// Settings offers 2/5/10); used when the setting is unset. Shared with the
/// pool's reaper tick.
pub(crate) const DEFAULT_IDLE_REAP_MINUTES: u32 = 5;

/// PRD-locked default model for the HTTP cut planner (single source: engine).
const DEFAULT_PLANNER_MODEL: &str = katto_engine::planner::http::DEFAULT_MODEL;

/// Which credentials exist in the keychain — presence only, never values.
#[derive(Debug, Clone, Serialize, Type)]
pub struct KeysPresent {
    pub elevenlabs: bool,
    pub anthropic: bool,
}

/// The app's settings as the frontend sees them, assembled from the key/value
/// `settings` table. `default_nle` stays `None` until the first export seeds it
/// (Phase 5). `capture_shortcut` is read-only here — rebinds go through
/// `set_capture_shortcut`, which must re-register the hotkey, not just persist.
#[derive(Debug, Clone, Serialize, Type)]
pub struct Settings {
    pub studio_root: Option<String>,
    pub default_nle: Option<String>,
    pub idle_reap_minutes: u32,
    pub onboarding_complete: bool,
    pub claude_path: Option<String>,
    pub capture_shortcut: String,
    pub planner_model: String,
    /// Nightly-curation discovery sweep toggle (`"true"`/`"false"` in the k/v
    /// table; default off — it needs uv and a hyper-frames checkout).
    pub discovery_enabled: bool,
    pub hyperframes_path: Option<String>,
    /// Cut planning through a visible dock session (default on; off = the
    /// Phase-4 subprocess planner).
    pub dock_planning: bool,
    /// ISO timestamp of the last studio.db import apply (None = never run);
    /// written by the import job, rendered as the wizard's "already
    /// imported" note.
    pub studio_import_last_run: Option<String>,
    pub keys_present: KeysPresent,
}

/// Partial update — only the `Some` fields are written.
#[derive(Debug, Clone, Deserialize, Type)]
pub struct SettingsPatch {
    pub studio_root: Option<String>,
    pub default_nle: Option<String>,
    pub idle_reap_minutes: Option<u32>,
    pub onboarding_complete: Option<bool>,
    pub claude_path: Option<String>,
    pub planner_model: Option<String>,
    pub discovery_enabled: Option<bool>,
    pub hyperframes_path: Option<String>,
    pub dock_planning: Option<bool>,
}

fn read_settings(conn: &Connection, keys_present: KeysPresent) -> Result<Settings> {
    Ok(Settings {
        studio_root: repo::get(conn, "studio_root")?,
        default_nle: repo::get(conn, "default_nle")?,
        idle_reap_minutes: repo::get(conn, "idle_reap_minutes")?
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_IDLE_REAP_MINUTES),
        onboarding_complete: repo::get(conn, "onboarding_complete")?.as_deref() == Some("true"),
        claude_path: repo::get(conn, "claude_path")?,
        capture_shortcut: repo::get(conn, "capture_shortcut")?
            .unwrap_or_else(|| crate::capture::DEFAULT_CAPTURE_SHORTCUT.to_string()),
        planner_model: repo::get(conn, "planner_model")?
            .unwrap_or_else(|| DEFAULT_PLANNER_MODEL.to_string()),
        discovery_enabled: repo::get(conn, "discovery_enabled")?.as_deref() == Some("true"),
        hyperframes_path: repo::get(conn, "hyperframes_path")?,
        dock_planning: repo::get(conn, "dock_planning")?.as_deref() != Some("false"),
        studio_import_last_run: repo::get(conn, "studio_import_last_run")?,
        keys_present,
    })
}

fn apply_patch(conn: &Connection, patch: &SettingsPatch) -> Result<()> {
    if let Some(v) = &patch.studio_root {
        repo::set(conn, "studio_root", v)?;
    }
    if let Some(v) = &patch.default_nle {
        repo::set(conn, "default_nle", v)?;
    }
    if let Some(v) = patch.idle_reap_minutes {
        repo::set(conn, "idle_reap_minutes", &v.to_string())?;
    }
    if let Some(v) = patch.onboarding_complete {
        repo::set(
            conn,
            "onboarding_complete",
            if v { "true" } else { "false" },
        )?;
    }
    if let Some(v) = &patch.claude_path {
        repo::set(conn, "claude_path", v)?;
    }
    if let Some(v) = &patch.planner_model {
        repo::set(conn, "planner_model", v)?;
    }
    if let Some(v) = patch.discovery_enabled {
        repo::set(conn, "discovery_enabled", if v { "true" } else { "false" })?;
    }
    if let Some(v) = &patch.hyperframes_path {
        repo::set(conn, "hyperframes_path", v)?;
    }
    if let Some(v) = patch.dock_planning {
        repo::set(conn, "dock_planning", if v { "true" } else { "false" })?;
    }
    Ok(())
}

async fn read_keys_present() -> Result<KeysPresent> {
    tauri::async_runtime::spawn_blocking(|| {
        // Presence is advisory — a keychain read failure (e.g. a denied
        // dev-build prompt) must never take down the settings surface, so
        // errors degrade to "absent" instead of failing the command.
        KeysPresent {
            elevenlabs: keychain::key_present(KeyService::Elevenlabs).unwrap_or(false),
            anthropic: keychain::key_present(KeyService::Anthropic).unwrap_or(false),
        }
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))
}

#[tauri::command]
#[specta::specta]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings> {
    let keys = read_keys_present().await?;
    state.db.call(move |conn| read_settings(conn, keys)).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_settings(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> Result<Settings> {
    let keys = read_keys_present().await?;
    let new_root = patch.studio_root.clone();
    let settings = state
        .db
        .call(move |conn| {
            apply_patch(conn, &patch)?;
            read_settings(conn, keys)
        })
        .await?;
    // A new studio root must be reachable over the asset protocol immediately
    // (footage playback in the review surface), not only after a relaunch.
    if let Some(root) = new_root {
        crate::assets::allow_studio_root(&app, &root);
    }
    Ok(settings)
}

/// Rebind the quick-capture hotkey: validate, swap the OS registration, then
/// persist — in that order, so a combo another app owns is rejected before
/// anything is written and the old binding stays live. If the DB write fails
/// after a successful swap, the registration is rolled back; a rollback that
/// itself fails leaves the OS on the new combo while settings keep the old one,
/// and that divergence is recorded as a `capture_hotkey_unavailable` events row
/// rather than silently.
#[tauri::command]
#[specta::specta]
pub async fn set_capture_shortcut(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    accel: String,
) -> Result<Settings> {
    let accel = accel.trim().to_ascii_lowercase();
    crate::capture::validate_accelerator(&accel)?;
    let keys = read_keys_present().await?;

    let old = state
        .db
        .call(|conn| repo::get(conn, "capture_shortcut"))
        .await?
        .unwrap_or_else(|| crate::capture::DEFAULT_CAPTURE_SHORTCUT.to_string());

    if old != accel {
        crate::capture::rebind_capture_hotkey(&app, &old, &accel).map_err(|err| {
            Error::ShortcutUnavailable(format!(
                "'{accel}' is unavailable — another app may own it: {err}"
            ))
        })?;
    }

    let persisted = {
        let accel = accel.clone();
        state
            .db
            .call(move |conn| {
                repo::set(conn, "capture_shortcut", &accel)?;
                read_settings(conn, keys)
            })
            .await
    };
    if persisted.is_err()
        && old != accel
        && crate::capture::rebind_capture_hotkey(&app, &accel, &old).is_err()
    {
        let detail = format!(
            "capture hotkey left on '{accel}' while settings kept '{old}' after a failed save"
        );
        let _ = state
            .db
            .call(move |conn| {
                crate::db::events::record(conn, "capture_hotkey_unavailable", None, Some(&detail))
            })
            .await;
    }
    persisted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_db;

    fn empty_patch() -> SettingsPatch {
        SettingsPatch {
            studio_root: None,
            default_nle: None,
            idle_reap_minutes: None,
            onboarding_complete: None,
            claude_path: None,
            planner_model: None,
            discovery_enabled: None,
            hyperframes_path: None,
            dock_planning: None,
        }
    }

    fn no_keys() -> KeysPresent {
        KeysPresent {
            elevenlabs: false,
            anthropic: false,
        }
    }

    #[test]
    fn defaults_on_a_fresh_db() {
        let conn = test_db();
        let s = read_settings(&conn, no_keys()).unwrap();
        assert_eq!(s.studio_root, None);
        assert_eq!(s.default_nle, None);
        assert_eq!(s.idle_reap_minutes, DEFAULT_IDLE_REAP_MINUTES);
        assert!(!s.onboarding_complete);
        assert_eq!(s.claude_path, None);
        assert!(!s.keys_present.elevenlabs);
        assert!(!s.keys_present.anthropic);
    }

    #[test]
    fn idle_reap_parses_when_set_and_falls_back_on_garbage() {
        let conn = test_db();
        repo::set(&conn, "idle_reap_minutes", "15").unwrap();
        assert_eq!(
            read_settings(&conn, no_keys()).unwrap().idle_reap_minutes,
            15
        );

        repo::set(&conn, "idle_reap_minutes", "not-a-number").unwrap();
        assert_eq!(
            read_settings(&conn, no_keys()).unwrap().idle_reap_minutes,
            DEFAULT_IDLE_REAP_MINUTES
        );
    }

    #[test]
    fn onboarding_complete_maps_only_true() {
        let conn = test_db();
        repo::set(&conn, "onboarding_complete", "true").unwrap();
        assert!(read_settings(&conn, no_keys()).unwrap().onboarding_complete);

        repo::set(&conn, "onboarding_complete", "false").unwrap();
        assert!(!read_settings(&conn, no_keys()).unwrap().onboarding_complete);
    }

    #[test]
    fn planner_model_defaults_and_reads_stored_value() {
        let conn = test_db();
        assert_eq!(
            read_settings(&conn, no_keys()).unwrap().planner_model,
            DEFAULT_PLANNER_MODEL
        );
        apply_patch(
            &conn,
            &SettingsPatch {
                planner_model: Some("claude-opus-4-8".to_string()),
                ..empty_patch()
            },
        )
        .unwrap();
        assert_eq!(
            read_settings(&conn, no_keys()).unwrap().planner_model,
            "claude-opus-4-8"
        );
    }

    #[test]
    fn capture_shortcut_falls_back_to_default() {
        let conn = test_db();
        assert_eq!(
            read_settings(&conn, no_keys()).unwrap().capture_shortcut,
            crate::capture::DEFAULT_CAPTURE_SHORTCUT
        );
    }

    #[test]
    fn capture_shortcut_reads_stored_value() {
        let conn = test_db();
        repo::set(&conn, "capture_shortcut", "ctrl+shift+j").unwrap();
        assert_eq!(
            read_settings(&conn, no_keys()).unwrap().capture_shortcut,
            "ctrl+shift+j"
        );
    }

    #[test]
    fn apply_patch_writes_only_some_fields() {
        let conn = test_db();
        apply_patch(
            &conn,
            &SettingsPatch {
                studio_root: Some("/Volumes/Studio".to_string()),
                ..empty_patch()
            },
        )
        .unwrap();
        let s = read_settings(&conn, no_keys()).unwrap();
        assert_eq!(s.studio_root.as_deref(), Some("/Volumes/Studio"));
        assert_eq!(s.idle_reap_minutes, DEFAULT_IDLE_REAP_MINUTES);

        apply_patch(
            &conn,
            &SettingsPatch {
                idle_reap_minutes: Some(20),
                onboarding_complete: Some(true),
                ..empty_patch()
            },
        )
        .unwrap();
        let s = read_settings(&conn, no_keys()).unwrap();
        assert_eq!(s.studio_root.as_deref(), Some("/Volumes/Studio"));
        assert_eq!(s.idle_reap_minutes, 20);
        assert!(s.onboarding_complete);
    }
}
