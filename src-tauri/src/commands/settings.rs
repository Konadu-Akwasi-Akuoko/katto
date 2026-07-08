use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

use crate::db::settings as repo;
use crate::error::Error;
use crate::error::Result;
use crate::keychain::{self, KeyService};
use crate::state::AppState;

/// Minutes of inactivity before a dock session is reaped (Phase 6); default when
/// the setting is unset.
const DEFAULT_IDLE_REAP_MINUTES: u32 = 10;

/// Which credentials exist in the keychain — presence only, never values.
#[derive(Debug, Clone, Serialize, Type)]
pub struct KeysPresent {
    pub elevenlabs: bool,
    pub anthropic: bool,
}

/// The app's settings as the frontend sees them, assembled from the key/value
/// `settings` table. `default_nle` stays `None` until the first export seeds it
/// (Phase 5).
#[derive(Debug, Clone, Serialize, Type)]
pub struct Settings {
    pub studio_root: Option<String>,
    pub default_nle: Option<String>,
    pub idle_reap_minutes: u32,
    pub onboarding_complete: bool,
    pub claude_path: Option<String>,
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
    Ok(())
}

async fn read_keys_present() -> Result<KeysPresent> {
    tauri::async_runtime::spawn_blocking(|| {
        Ok(KeysPresent {
            elevenlabs: keychain::key_present(KeyService::Elevenlabs)?,
            anthropic: keychain::key_present(KeyService::Anthropic)?,
        })
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))?
}

#[tauri::command]
#[specta::specta]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings> {
    let keys = read_keys_present().await?;
    state.db.call(move |conn| read_settings(conn, keys)).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_settings(state: State<'_, AppState>, patch: SettingsPatch) -> Result<Settings> {
    let keys = read_keys_present().await?;
    state
        .db
        .call(move |conn| {
            apply_patch(conn, &patch)?;
            read_settings(conn, keys)
        })
        .await
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
