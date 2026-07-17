---
paths:
  - "src-tauri/src/**"
---

# Tauri app-crate rules

## Composition root

- `main.rs` stays a 3-line entry point calling `katto_lib::run()`. All real code lives under `lib.rs`.
- `lib.rs` is a composition root only: plugin registration (single-instance first), `.manage()` state, one `generate_handler![...]`, setup hook. No business logic.
- Domain folders, not god-files: `commands/<domain>.rs`, `db/<aggregate>.rs`, `jobs/`, `state.rs`, `error.rs`. One commands module per domain.

## Commands

- Names are `snake_case` verb phrases, globally unique, one command per user-facing action: `get_settings`, `promote_idea`, `start_ingest_job`. Never a generic `dispatch(action, payload)`.
- Every fallible command returns `Result<T, Error>` where `Error` is the app-wide thiserror enum with a hand-written `Serialize` impl emitting a tagged shape (`{kind, message}`) so the frontend can discriminate. Never `Result<T, String>` or scattered `.map_err(|e| e.to_string())`.
- Commands doing I/O are `async fn`; wrap synchronous/CPU-heavy work (rusqlite, ffprobe parsing) in `tauri::async_runtime::spawn_blocking`. Async commands take owned params (`String`, not `&str`).
- Managed state via `state: State<'_, AppState>`; interior mutability lives inside the state type (`std::sync::Mutex` unless held across `.await`, then `tokio::sync::Mutex`).
- Command-scoped streaming (job progress, PTY output, planner tokens) uses `tauri::ipc::Channel<T>` parameters. App-wide broadcast (tray state, drive mounted/unmounted) uses events. Never poll.
- Commands are thin shells: unwrap args → call engine/db/jobs layer → map errors. No SQL in commands, ever — `db/` repository modules take `&Connection` and own all queries.
- Media bytes never cross `invoke`: video/audio/thumbnails go through the asset protocol (`convertFileSrc`).

## Persistence

- rusqlite behind a single dedicated writer (channel-fed task); WAL + `busy_timeout` pragmas set on open.
- Migrations are numbered and forward-only (`rusqlite_migration`); never edit a shipped migration — add a new one. Column additions are a plain `ALTER TABLE … ADD COLUMN` — no presence guard: the ladder's `user_version` already applies each entry exactly once, and plain SQLite cannot branch on `PRAGMA table_info` anyway.
- File writes that replace project artifacts are atomic: write `<name>.tmp`, then `rename`. Versioned exports (`timelines/<slug>-vN.*`) are never overwritten — bump N.

## Jobs and events

- Every background operation registers a `jobs` row, streams progress over its `Channel`, mirrors state to the tray, and writes an `events` row on terminal state. Nothing fails silently; errors surface via events, never blocking dialogs.
