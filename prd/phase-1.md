# Phase 1 — Shell & First Light

## Goal

An installable `.app` that lives in the menu bar, opens a real (if sparse) main window,
completes onboarding, and owns the persistence + jobs + events infrastructure every later
phase builds on.

## Why this order

Everything downstream needs the shell: the tray is where progress surfaces, the jobs/events
framework is the "nothing fails silently" backbone (D18), the palette is the app's command
spine, onboarding establishes the three facts katto cannot guess (studio root, ElevenLabs key,
claude path). Building it first also forces the frontend toolchain and typed
IPC layer into place before any feature code exists to do it wrong.

## User stories

- As the owner, I launch katto and it appears in the menu bar; clicking the tray icon toggles
  the main window; closing the window sends it back to the tray, still running.
- On first run, a wizard walks me through picking the Studio root — an external SSD (recommended)
  or a local folder; pointing it at my Mac's internal drive warns me but lets me proceed — pasting
  my ElevenLabs key, and confirming `claude` was found on PATH (or pasting an Anthropic key).
- If a removable Studio root is unplugged, the app tells me plainly and recovers the moment I reconnect.
- I press ⌘K anywhere and see every available action.
- Anything katto does in the background shows up as a job with live progress and lands in an
  activity feed I can read later.

## Scope with acceptance criteria

| Feature | Acceptance criteria |
|---|---|
| Tray residency | Template icon (dark/light correct); `ActivationPolicy::Regular` — Dock icon + macOS app menu present (matches real Docker Desktop, which shows a Dock icon; supersedes the old Accessory decision — see D21); **left-click opens the tray menu** (does not touch the window); menu shows a dynamic **Show window / Hide window** item that flips with window state, current-project line (— for now), next-shoot-day line (— for now), Quit; menu items update live without rebuilding the tray; window opens **maximized** (fills the work area, not fullscreen); **closing destroys the WebView** (frees its RAM) while the app stays resident in the menu bar (`RunEvent::ExitRequested` → `prevent_exit` when `code` is `None`); reopening from the tray item, the Dock icon (`RunEvent::Reopen`), or a second launch recreates the maximized window; tray **Quit** (`app.exit`, `code` `Some`) exits fully |
| Launch at login | Toggle in Settings; survives reboot when enabled from a bundled build |
| Single instance | Second launch focuses the existing window |
| SQLite bootstrap | DB created at `app_data_dir()/katto.db` with WAL + `busy_timeout=5000` + `synchronous=NORMAL`; full schema below applied via numbered migrations; migrations test green |
| Events log | `record_event(kind, project_slug?, payload)` writer; `list_events(limit, before?)` query; append-only (no update/delete paths) |
| Jobs framework | `Job` = row + state machine `queued→running→(done\|failed)`; progress 0–1 streamed over `Channel<JobProgress>`; terminal state writes an `events` row; tray menu mirrors the active job |
| Settings | SQLite `settings` table (key/value); typed accessors for `studio_root`, `default_nle`, `idle_reap_minutes`, `onboarding_complete`; no plugin-store. `default_nle` is left UNSET by onboarding — it is seeded lazily at the first export (Phase 5) and sticky thereafter |
| Keychain | `store_key(service, value)` / `key_present(service)` for `elevenlabs` and `anthropic` under service `katto`; values never returned to the frontend, never logged |
| Studio-root picker | Any directory allowed; onboarding + Settings warn (non-blocking) when the chosen root resolves to the boot volume or has < 100 GB free — camera footage is large, external SSD recommended; the user may proceed regardless |
| Studio-root mount check | On launch + volume events: root path unreachable → app-wide "drive disconnected" banner state with the expected path; auto-clears on remount (a root on the internal drive is effectively always present, so the banner is a no-op there) |
| `claude` detection | `which claude` via login shell (`zsh -lc`); result cached in settings; re-run from Settings |
| Onboarding wizard | Runs when `onboarding_complete` unset; three steps above; completing writes settings + keychain and lands on the Dashboard |
| ⌘K palette | cmdk overlay; command registry module where features register `{id, title, keywords, action}`; Phase-1 commands: open settings, open dashboard, quit, sleep to tray, re-run claude detection |
| Dashboard v1 | Events feed (latest 50), active-jobs list with progress bars, drive status card |
| Frontend toolchain | Tailwind v4 (CSS-first `@theme` in `src/styles/main.css`), Biome v2, Vitest + RTL + `@tauri-apps/api/mocks` setup, TanStack Query + Zustand, shadcn CLI initialized (`components/ui/`), tauri-specta bindings generating `src/lib/ipc/bindings.gen.ts`, tsconfig gains `noUncheckedIndexedAccess` + `verbatimModuleSyntax`, `@/` alias |
| Gate/CI extension | `justfile` gains `biome`, `vitest`, real `bindings` recipes; `just check` = fmt-check + clippy + cargo test + biome + tsc + vitest; CI rust job moves to workspace root and mirrors `just check`; note in CI config: switch runner ubuntu→macos when objc2/macOS-only deps land (Phase 2) |

## Backend (Rust)

Layout (domain folders per `.claude/rules/tauri-commands.md`):

```
src-tauri/src/
  lib.rs            # composition root only
  main.rs           # 3-line entry
  error.rs          # app Error enum (thiserror + tagged Serialize {kind, message})
  state.rs          # AppState { db: DbHandle, settings cache, tray handles }
  tray.rs           # TrayIconBuilder setup + LiveState → menu-item updates
  db.rs + db/       # connection (single writer task, channel-fed), migrations.rs,
                    #   settings.rs, events.rs, jobs.rs repositories
  jobs.rs + jobs/   # Job runtime: registry, progress channels, tray mirror
  commands.rs + commands/  # settings.rs, events.rs, jobs.rs, onboarding.rs
  keychain.rs       # keyring-core + apple-native-keyring-store wrapper
  paths.rs          # studio-root resolution + mount check + boot-volume/free-space warning
```

Crates: `tauri 2.11` (pin minor; features `tray-icon`, `image-png`), `tauri-plugin-single-instance`
(registered first), `tauri-plugin-autostart` (`MacosLauncher::AppleScript` — LaunchAgent mode has
open macOS bugs), `tauri-plugin-dialog`, `rusqlite 0.40 bundled`, `rusqlite_migration`,
`keyring-core` + `apple-native-keyring-store` (fallback: `keyring 3.6` `apple-native`),
`thiserror`, `specta` + `tauri-specta 2.0.0-rc` (pinned exact), `tokio` (workspace async).

Tray gotchas (verified): keep `MenuItem` handles and call `set_text` on the main thread
(`app.run_on_main_thread`); runtime `set_icon` resets the template flag — re-call
`set_icon_as_template(true)` after any icon swap.

Keychain gotcha: unsigned dev builds re-prompt on every rebuild; sign dev builds with a stable
identity or tolerate prompts until Phase 7 packaging.

## Frontend (React)

```
src/
  app/                # App.tsx (providers, shell layout), providers.tsx
  features/
    dashboard/        # events feed, jobs list, drive status card
    onboarding/       # wizard (3 steps), gate component
    settings/         # settings page (root, NLE, idle-reap, keys, autostart, claude detect)
    palette/          # cmdk overlay + command registry (lib-like: features import register())
  components/ui/      # shadcn copy-in primitives
  components/layout/  # sidebar, titlebar, banner (drive-disconnected)
  lib/ipc/            # bindings.gen.ts (generated) + settings.ts, events.ts, jobs.ts wrappers
  lib/query-client.ts # TanStack Query client + global MutationCache error→toast mapping
  stores/ui.ts        # sidebar/palette/banner UI state (zustand, selectors only)
  styles/main.css     # Tailwind v4 @theme tokens (semantic: --color-surface, --color-accent…)
  test/setup.ts       # vitest setup: mocks plumbing, clearMocks() afterEach, crypto polyfill
```

Sidebar: Dashboard, Planner (stub), Projects (stub), Settings + a reserved Claude-dock icon
slot (Phase 6). Main region renders the active surface.

## Wiring / IPC

All commands specta-typed; frontend consumes only `lib/ipc/*` wrappers.

| Command | Signature (conceptual) | Notes |
|---|---|---|
| `get_settings` | `() -> Settings` | `Settings {studio_root?, default_nle, idle_reap_minutes, onboarding_complete, claude_path?, keys_present: {elevenlabs, anthropic}}` |
| `set_settings` | `(patch: SettingsPatch) -> Settings` | partial update |
| `pick_studio_root` | `() -> Option<String>` | dialog plugin folder picker; validates writable |
| `store_key` | `(service: KeyService, value: String) -> ()` | writes keychain; value never echoed back |
| `detect_claude` | `() -> Option<String>` | `zsh -lc 'which claude'` |
| `complete_onboarding` | `() -> ()` | sets flag + emits event |
| `list_events` | `(limit: u32, before_id: Option<i64>) -> Vec<Event>` | |
| `list_jobs` | `(active_only: bool) -> Vec<Job>` | |
| `subscribe_job_progress` | `(job_id, on_progress: Channel<JobProgress>) -> ()` | `JobProgress {job_id, progress: f32, message?}` |
| `set_autostart` | `(enabled: bool) -> ()` | autostart plugin |

Events (broadcast): `drive-status-changed {mounted: bool, path}`, `jobs-changed`,
`events-appended` (dashboard invalidation), tray→window `show-window`.

## Data-model deltas (initial schema, migration 001)

```sql
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE TABLE projects (
  slug         TEXT PRIMARY KEY,
  title        TEXT NOT NULL,
  root_path    TEXT NOT NULL,
  status       TEXT NOT NULL DEFAULT 'idea',
  target_nle   TEXT NOT NULL DEFAULT 'fcp',
  shoot_date   TEXT,
  publish_date TEXT,
  created_at   TEXT NOT NULL
);

-- ideas & raw_signal: column parity with hyper-frames tools/studio (D7).
CREATE TABLE ideas (
  id            TEXT PRIMARY KEY,
  type          TEXT NOT NULL,                 -- mirror|comment_demand|trend|manual
  kind          TEXT NOT NULL DEFAULT 'unset', -- unset|long|short|series
  status        TEXT NOT NULL DEFAULT 'backlog', -- backlog|promoted|discarded
  title         TEXT NOT NULL,
  rationale     TEXT,
  source        TEXT,
  source_url    TEXT,
  source_title  TEXT,
  evidence_json TEXT,
  raw_signal_id TEXT,
  first_seen    TEXT NOT NULL,
  notes         TEXT,
  promoted_slug TEXT,
  kind_source   TEXT,                          -- 'ai' | 'human'
  kind_why      TEXT
);
CREATE INDEX idx_ideas_status ON ideas(status);

CREATE TABLE raw_signal (
  id             TEXT PRIMARY KEY,
  source         TEXT NOT NULL,
  external_id    TEXT NOT NULL,
  title          TEXT,
  url            TEXT,
  payload_json   TEXT NOT NULL,
  fetched_at     TEXT NOT NULL,
  judged_at      TEXT,
  judged_verdict TEXT
);
CREATE INDEX idx_raw_unjudged ON raw_signal(judged_at) WHERE judged_at IS NULL;

CREATE TABLE schedule (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  project_slug TEXT NOT NULL REFERENCES projects(slug) ON DELETE CASCADE,
  kind         TEXT NOT NULL,                  -- shoot|publish
  date         TEXT NOT NULL,
  note         TEXT
);

CREATE TABLE events (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  ts           TEXT NOT NULL,
  kind         TEXT NOT NULL,
  project_slug TEXT,
  payload_json TEXT
);

CREATE TABLE jobs (
  id           TEXT PRIMARY KEY,
  kind         TEXT NOT NULL,
  label        TEXT NOT NULL,
  status       TEXT NOT NULL DEFAULT 'queued', -- queued|running|done|failed
  progress     REAL NOT NULL DEFAULT 0,
  payload_json TEXT,
  error        TEXT,
  started_at   TEXT,
  finished_at  TEXT
);

CREATE TABLE scheduled_jobs (
  name            TEXT PRIMARY KEY,
  spec            TEXT NOT NULL,
  last_success_at TEXT,
  enabled         INTEGER NOT NULL DEFAULT 1
);
```

Idea-status vocabulary note: katto uses `backlog|promoted|discarded` (D7); the Phase-7 import
maps studio's `new|keep→backlog`, `rejected→discarded`, `promoted→promoted`.

## Error handling

- Keychain write/read failure → typed `Error::Keychain {message}`; onboarding step shows it
  inline and allows retry; never blocks the rest of the wizard.
- DB open/migration failure → fatal with a plain dialog naming the DB path (the one permitted
  blocking dialog: the app cannot run without its DB).
- Studio root missing → not an error: the explicit disconnected banner state (drive watcher
  clears it). All feature commands touching the root return `Error::StudioRootUnmounted`.
- Job failures set `status='failed'` + `error`, write an `events` row, and keep the job
  visible in the dashboard — no toast-and-forget.

## Testing

- `migrations_apply_on_fresh_memory_db` (mandatory, per `.claude/rules/testing.md`).
- `db/` repositories: in-memory DB tests (settings round-trip, event append+list ordering,
  jobs state transitions reject invalid moves e.g. `done→running`).
- Jobs framework: state-machine unit tests with a stub channel.
- Frontend: palette registry unit tests; onboarding wizard step-flow test with `mockIPC`;
  dashboard renders events/jobs fixtures.
- Manual: bundled build → tray present, onboarding completes, quit/relaunch retains settings,
  autostart works, unplugging the SSD flips the banner.

## Out of scope

Planner/Projects surfaces (Phase 2), any ingest (3), any engine work (4–5), notifications and
deep links (Phase 2 — needs `objc2-user-notifications`), the scheduler runtime (6).

## Exit criteria

Installable `.app` lives in the menu bar; onboarding completes and persists; `just check`
(now including biome + vitest) green; CI mirrors it.
