use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::error::{Error, Result};
use crate::sessions::{Program, SessionTask};
use crate::state::AppState;
use crate::vfx::{VfxEffect, effect_slug, list_effects};

/// Scaffold `assets/vfx/<slug>/` for a project and open a dock session in it.
/// Returns the session id so the frontend can focus the dock on it (the
/// folder path is derivable from the slug).
#[tauri::command]
#[specta::specta]
pub async fn create_vfx_effect(
    app: AppHandle,
    state: State<'_, AppState>,
    project_slug: String,
    name: String,
) -> Result<String> {
    let slug = effect_slug(&name).ok_or_else(|| {
        Error::InvalidName(format!(
            "'{name}' has no usable characters for a folder name"
        ))
    })?;
    let project = {
        let project_slug = project_slug.clone();
        state
            .db
            .call(move |conn| crate::db::projects::get(conn, &project_slug))
            .await?
    }
    .ok_or_else(|| Error::NoSuchProject(format!("no project {project_slug}")))?;

    let effect_dir = PathBuf::from(&project.root_path)
        .join("assets/vfx")
        .join(&slug);
    {
        let effect_dir = effect_dir.clone();
        tauri::async_runtime::spawn_blocking(move || std::fs::create_dir_all(&effect_dir))
            .await
            .map_err(|e| Error::Io(e.to_string()))??;
    }

    {
        let payload = serde_json::json!({ "project": project_slug, "effect": slug }).to_string();
        let slug_for_event = project_slug.clone();
        state
            .db
            .call(move |conn| {
                crate::db::events::record(
                    conn,
                    "vfx_effect_created",
                    Some(&slug_for_event),
                    Some(&payload),
                )
            })
            .await?;
        crate::broadcast::events_appended(&app);
    }

    let task = SessionTask {
        label: format!("vfx: {slug}"),
        cwd: effect_dir,
        initial_prompt: Some(format!(
            "This is the VFX workspace for the effect \"{name}\" of the video project \
             \"{title}\". Build it here with your usual HyperFrames/Remotion toolchain; \
             render output lands in this folder.",
            title = project.title,
        )),
        append_system_prompt: None,
        permission_mode: None,
        permission_allow: vec![],
    };
    state.sessions.spawn(&app, task, Program::Claude).await
}

/// Every effect folder of a project with its renders (folders are truth).
#[tauri::command]
#[specta::specta]
pub async fn list_vfx_effects(
    state: State<'_, AppState>,
    project_slug: String,
) -> Result<Vec<VfxEffect>> {
    let project = {
        let project_slug = project_slug.clone();
        state
            .db
            .call(move |conn| crate::db::projects::get(conn, &project_slug))
            .await?
    }
    .ok_or_else(|| Error::NoSuchProject(format!("no project {project_slug}")))?;
    let root = PathBuf::from(project.root_path);
    tauri::async_runtime::spawn_blocking(move || list_effects(&root))
        .await
        .map_err(|e| Error::Io(e.to_string()))
}
