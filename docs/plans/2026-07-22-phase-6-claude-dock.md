# Phase 6 — Claude Dock & Automations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (or
> superpowers:subagent-driven-development) to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every AI task becomes a visible, interruptible Claude Code session in a slide-over
dock panel (PTY-backed, xterm.js, typed-into mid-run); katto gains an anacron-style scheduler
with catch-up; nightly idea curation and the VFX cockpit go live; cut planning re-routes
through the dock (PRD: `prd/phase-6.md`).

**Architecture:** All session machinery lives in the app crate (`src-tauri/src/sessions/` —
pool, PTY, state machine, hooks endpoint, reaper), because sessions are app orchestration,
not media math; `katto-engine` is untouched this phase. Pure logic (state transitions,
scrollback ring, launch command/settings assembly, scheduler due-math, reap decision, VFX
event classification) is TDD'd as plain functions; the PTY/process/watch spawn sites stay
thin. Session state is primarily hook-driven: each spawned `claude` gets a katto-generated
`--settings` JSON whose `Stop`/`Notification` hooks POST to a token-authed localhost
endpoint; output-silence heuristics are the degraded fallback. The frontend adds
`features/dock/` (slide-over panel, tab strip, xterm terminal, sidebar icon states) and
`features/vfx/`; curated ideas surface in the existing Planner backlog with
rationale/provenance treatment.

**Tech Stack:** Rust 2024 (portable-pty 0.9.0, tiny_http 0.12.0, objc2-app-kit 0.3.2
`NSWorkspace` feature, chrono, existing notify 8.2), Tauri 2 + tauri-specta `=2.0.0-rc.25`,
React 19 + TS + `@xterm/xterm` 6.0.0 + `@xterm/addon-fit` 0.11.0 + `@xterm/addon-webgl`
0.19.0, vitest + RTL, bun.

## Global Constraints

- **Gate:** `just check` (fmt-check + clippy `-D warnings` + cargo test + biome + tsc +
  vitest) from the workspace root. Never claim a task or the phase done without it green.
- **Every session and every scheduled run is a `jobs` row + `events` row** (D18). Session
  spawn → jobs row kind `claude_session` (running for the session's lifetime); scheduled
  curation → jobs row kind `nightly_curation`. PTY death, scheduler misfire, curation parse
  failure, discovery failure, hooks degradation — all get events rows; nothing fails
  silently or crash-loops.
- **No numeric scoring anywhere** (D7): curation verdicts are binary keep/discard with a
  one-line rationale; `lean` is the categorical `hold|lean|strong` notch (a meter hint,
  rendered as a notch fill, never a number/percent/grade). Sessions never auto-act on
  ideas — AI suggests, the human decides (promote/discard stays a human tap).
- **Secrets never logged:** sessions run on the owner's subscription auth (`zsh -lc`
  forwards the login environment); katto never injects `ANTHROPIC_API_KEY` into a session,
  never writes env contents into events/jobs rows or logs, and the hooks token appears only
  in the generated per-session settings file (0600) and memory.
- **Media bytes never cross `invoke`** — applies to MEDIA. PTY terminal bytes are not
  media: they stream as `Channel<Vec<u8>>` batches (~16 ms / 16 KB flushes). VFX render
  previews go through `convertFileSrc` (asset protocol) as always.
- **Versioned artifacts / atomic writes:** unchanged invariants; the dock cut-planning path
  still lands `cuts.json` via the bundle contract and never overwrites timelines.
- **Rational time end-to-end** in the engine — this phase adds no engine time math; do not
  introduce float time anywhere backend.
- **Dirty-tree discipline:** `src/components/ui/date-input.{tsx,test.tsx}`, hunks in
  `src/features/projects/detail/project-detail.{tsx,test.tsx}` and one hunk in
  `src/styles/main.css` are leftover owner DateInput work — **never commit them, never
  `git add -A`**. Tasks 9 (main.css tokens) and 18 (project-detail.tsx) touch those files:
  `git stash push src/components/ui/date-input.tsx src/components/ui/date-input.test.tsx src/features/projects/detail/project-detail.tsx src/features/projects/detail/project-detail.test.tsx src/styles/main.css`,
  commit, `git stash pop`; verify with `git diff --cached` that no DateInput/owner hunk is
  staged (owner-hunk reference: `/private/tmp/claude-501/-Users-akwasikonaduakuoko-Projects-Rust-katto/8162840d-7572-4982-97cd-e5f8b2f02e74/scratchpad/dateinput-backup/tracked.patch`).
  Never stage `CLAUDE.md`, `docs/overnight-run.md`, or `docs/plans/`.
- **Do not start the dev app**; the owner tests visually after waking. All UI-observable
  behavior lands as checkboxes in `docs/overnight-run.md` (Task 20).
- **Live-call budget:** at most 2–3 short real `claude` invocations for smoke-testing
  (subscription auth, short prompts, e.g. the `#[ignore]` smoke test in Task 6 run once).
  No paid API calls. All other tests use fake shells (`bash -c 'cat'` per the PRD).
- Conventional commits, one concern per commit, tests travel with their feature commit.
  Commits go through the **`ship` skill** with explicit paths. Frontend: bun only;
  regenerate bindings via the `export_bindings` test (`just check` runs it); never
  hand-edit `src/lib/ipc/bindings.gen.ts`. Use `add-tauri-command` before new commands,
  `add-db-migration` for Task 1, `add-feature-surface` for the dock feature folder.
- App-crate rules: commands are thin shells (no SQL — `db/` repos own queries); every
  fallible command returns `Result<T, Error>` (tagged `{kind, message}`); command-scoped
  streaming uses `Channel<T>`, app-wide broadcast uses tauri-specta events; interior
  mutability inside managed state; `std::sync::Mutex` unless held across `.await`. No
  `unwrap()`/`expect()` outside tests; 2018 module style (`sessions.rs` +
  `sessions/pool.rs`, never `mod.rs`).
- Design-system rules bind every UI task: tokens never literals (new tokens go in the
  `@theme` block), **mono is for machine output only** — xterm content is mono, all dock
  chrome/labels/tabs are sans; state shown once (chip or dot, never also a rail); no
  eyebrows/gradients/glassmorphism/bounce; `prefers-reduced-motion` gates the pulsing ring
  and slide animation; `cursor: default` on controls; grain only on opaque surfaces (the
  terminal viewport opts out of grain entirely).

## Verified external contracts (verified 2026-07-22 — do not re-derive)

- **portable-pty 0.9.0** (docs.rs): `native_pty_system() -> Box<dyn PtySystem>`;
  `pty_system.openpty(PtySize { rows: u16, cols: u16, pixel_width: u16, pixel_height: u16 })
  -> Result<PtyPair, Error>` (`PtyPair { master: Box<dyn MasterPty>, slave: Box<dyn SlavePty> }`);
  `CommandBuilder::new(program)` with `.arg()`, `.cwd(path)`, `.env(k, v)`;
  `slave.spawn_command(cmd) -> Result<Box<dyn Child>, Error>`;
  `master.try_clone_reader() -> Result<Box<dyn Read + Send>, Error>`;
  `master.take_writer() -> Result<Box<dyn Write + Send>, Error>`;
  `master.resize(PtySize) -> Result<(), Error>`; `Child` has `wait() -> Result<ExitStatus>`
  (blocking), `try_wait()`, and `clone_killer() -> Box<dyn ChildKiller + Send + Sync>` with
  `.kill()`. After spawn, **drop the `slave`** (and drop the master's writer on close) so
  EOF propagates. Reads are blocking `std::io::Read` — dedicated thread per session.
- **@xterm/xterm 6.0.0** (npm registry; `@xterm/addon-fit` 0.11.0, `@xterm/addon-webgl`
  0.19.0 are the current stable partners): `new Terminal({fontFamily, fontSize, theme:
  {background, foreground, cursor, selectionBackground}, scrollback, convertEol?})`;
  `term.open(el)`; `term.write(data: string | Uint8Array)`; `term.onData(cb: (s: string)
  => void)` (user keystrokes, already encoded — send verbatim to the PTY);
  `term.onResize(cb: ({cols, rows}) => void)`; `term.loadAddon(addon)`;
  `fitAddon.fit()`; `term.dispose()`. WebGL addon: `new WebglAddon()`, listen
  `addon.onContextLoss(() => addon.dispose())` — terminal falls back to the DOM renderer.
- **claude CLI (owner's install: 2.1.217; PATH via `zsh -lc`, found by
  `katto_engine::detect::detect_claude()`):**
  - `claude "initial prompt"` starts the **interactive** REPL with that prompt submitted.
  - `--settings <file-or-json>` exists and merges/overrides settings.json keys **for that
    session** — this is how katto injects hooks per session (verified against the CLI
    reference and `claude --help`).
  - `--append-system-prompt <text>` appends to the default system prompt (interactive OK).
  - `--permission-mode acceptEdits` exists; permission **allow rules** live under
    `{"permissions": {"allow": ["Bash(sqlite3:*)"]}}` in settings JSON. **[CHECK at
    implementation: exact `Bash(...)` matcher syntax against
    https://code.claude.com/docs/en/iam** — prefix rules use `:*` suffix.]
  - **Hooks** (verified against https://code.claude.com/docs/en/hooks): settings shape
    `{"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "<shell string>"}]}],
    "Notification": [{"matcher": "...", "hooks": [...]}]}}`. `Stop` fires when Claude
    finishes responding (end of each turn; no matcher). `Notification` fires with a
    `matcher` filter over notification types — `permission_prompt` (permission dialog) and
    `idle_prompt` are the needs-input signals. Hook command receives JSON on **stdin**
    (`{session_id, transcript_path, cwd, hook_event_name, message?, ...}`). Shell-form
    command (no `args`) runs through a shell — pipe stdin to `curl` verbatim. Notification
    hooks cannot block; Stop exit codes are ignored by us (always `|| true`).
- **tiny_http 0.12.0**: `Server::http("127.0.0.1:0")` binds an OS-assigned port;
  `server.server_addr()` yields the bound addr (`.to_ip().map(|a| a.port())`);
  `server.recv_timeout(Duration)` → `Option<Request>`; `request.headers()`,
  `request.as_reader()` (body), `request.respond(Response::empty(status))`. Synchronous —
  runs on its own thread like the DB writer.
- **objc2-app-kit 0.3.2** (already a dep at 0.3, features `["std","NSWindow","NSResponder"]`):
  add feature `"NSWorkspace"`. Items:
  `NSWorkspace::sharedWorkspace() -> Retained<NSWorkspace>`,
  `workspace.notificationCenter() -> Retained<NSNotificationCenter>`,
  `pub unsafe static NSWorkspaceDidWakeNotification: &'static NSNotificationName`.
  Block observer via objc2-foundation `NSNotificationCenter::addObserverForName_object_queue_usingBlock`
  (needs objc2-foundation `NSNotification` + `NSOperation` features and `block2` — block2
  0.6 already a dep). Keep the returned observer token `Retained` alive in managed state.
  **[CHECK exact objc2-foundation feature names at build — cargo errors name them.]**
- **objc2-user-notifications 0.3** (already a dep; `notify.rs` uses it): the click→route
  delegate needs `UNUserNotificationCenterDelegate` (protocol trait) implemented via
  `objc2::define_class!`, method
  `userNotificationCenter_didReceiveNotificationResponse_withCompletionHandler`, reading
  `response.notification().request().content().userInfo()`. **[CHECK exact generated
  method/trait names on docs.rs/objc2-user-notifications/0.3 at implementation; the
  feature flags needed (e.g. `UNUserNotificationCenter`, `UNNotificationResponse`) are
  named by cargo errors.]**
- **tauri 2 `Channel<Vec<u8>>`**: `tauri::ipc::Channel<T: Serialize>` serializes `Vec<u8>`
  as a JSON number array; tauri-specta types it `number[]` — frontend converts with
  `new Uint8Array(data)`. Acceptable at 16 KB/16 ms local IPC. (If profiling ever demands
  it, `InvokeResponseBody::Raw(Vec<u8>)` exists as the escape hatch — do NOT use it now;
  it bypasses specta typing.)
- **studio-discover CLI** (hyper-frames mirror,
  `hyper-frames/tools/studio/discovery/src/studio_discovery/cli.py`): invocation
  `uv run studio-discover --db <path>` from the `discovery/` directory; flags
  `--sources youtube,hn,reddit,lobsters,dailydev`, `--channels`, `--videos-per-channel`
  (default 15), `--comments-per-video` (default 30), `--comments-from-top`. Writes
  `raw_signal` rows; **zero AI, decides nothing**; one dead source logs and continues.
  katto's `raw_signal`/`ideas` tables have column parity (D7) so `--db <katto.db>` works
  unchanged.
- **Curation judgment source** (normative, from the committed mirror
  `hyper-frames/.claude/skills/studio-ideas/SKILL.md`): the three lenses (fit / novelty /
  demand shape), binary keep/discard with one-line *why-pursue* rationale, **never a
  number/score/rank/percentage/grade**, suggested `kind` (long/short/series) with
  `kind_source='ai'` + one-line `kind_why`, categorical `lean` `hold|lean|strong` in
  `evidence_json`, delta-only reads (`WHERE judged_at IS NULL`, batched, comments
  separately), mark every read row judged, discarded rows stay as audit trail. Reused
  verbatim in the Task 14 prompt with katto-schema deltas: `status='backlog'` (not
  `'new'`), novelty guard on `status IN ('backlog','promoted')` + `projects.slug` list
  (not `videos/` folders), and the summary is computed by katto from DB deltas (below).

## Locked design decisions (decided now — do not re-litigate)

1. **The "dock icon" is the in-app sidebar Claude icon**, not the tray. The PRD scope
   table says "Sidebar Claude icon with 4 states"; the sidebar already carries a disabled
   `RobotIcon` placeholder ("Arrives in Phase 6", `src/components/layout/sidebar.tsx:66`).
   The 4 states are CSS-driven on that button. The tray is untouched this phase (its `job`
   menu line already mirrors jobs). No new icon assets are needed.
2. **Hooks endpoint = tiny_http on its own thread**, not axum. Two routes, a handful of
   requests per hour, no async ceremony; mirrors the DB-writer thread pattern. PRD
   explicitly allows tiny_http.
3. **Hook transport = shell-form `command` hooks running `curl`**, generated per session
   into `<app_data_dir>/sessions/<id>.settings.json` (0600, deleted on close). The katto
   session id is baked into the URL path (`/hook/<id>`), so the endpoint never needs to
   map Claude-internal session ids. `hook_event_name` in the body discriminates
   Stop/Notification. Always `|| true` so a dead endpoint never breaks the session.
4. **Session states (backend):** `Running`, `NeedsInput`, `Idle`, `Failed`, `Closed`. The
   PRD's "done" is a **UI treatment**: the frontend flashes a check for ~3 s on a
   `running → idle` transition. Hook-driven transitions when hooks are live; output
   silence/burst heuristics only in degraded mode (endpoint never heard from the session
   → `session_hooks_degraded` events row, once).
5. **Curation summary comes from DB deltas, not output parsing:** katto records
   `run_started_at`, then after the session's first `Stop` counts
   `raw_signal WHERE judged_at >= run_started_at` grouped by verdict and
   `ideas WHERE first_seen >= run_started_at`. Strictly more robust than parsing terminal
   text, and trivially "tolerates absent summary" (PRD error-handling row).
6. **`scheduled_jobs.spec` is the single source of truth for schedule time/enabled** —
   the Settings page edits it via `set_scheduled_job`; no duplicate settings keys.
   Discovery toggle + hyper-frames path stay in the settings k/v (`discovery_enabled`,
   `hyperframes_path`) because they configure the run, not the schedule.
7. **Idle-reap default becomes 5 minutes** (PRD/D15: default 5, options 2/10):
   `DEFAULT_IDLE_REAP_MINUTES` in `commands/settings.rs` changes 10 → 5; Settings UI
   offers 2/5/10.
8. **Cut-plan dock path is gated by settings key `dock_planning`** (default `"true"`).
   Disabled or `claude` missing → the Phase-4 subprocess planner (unchanged fallback).
   Dock-path failure fails the pipeline stage visibly (D18) — no silent fallback.
9. **Terminal viewport gets two new semantic tokens** (`--term-bg`, `--term-fg`) in
   `src/styles/main.css` `@theme`: the terminal stays dark in both app themes (machine
   region, like every real terminal), warm-neutral near-black per the design system's
   chroma rule. No grain on the terminal. All other dock chrome uses existing tokens.
10. **PTY streaming pipeline:** per session, a blocking reader thread pushes raw chunks
    into an `mpsc`; a flusher thread drains with `recv_timeout(16 ms)` and flushes to the
    scrollback ring + the attached `Channel` when 16 KB accumulates or the timeout fires.
    Scrollback ring caps at 2 MiB; `attach_session` replays the ring first, then live
    batches. One attached channel per session (latest wins — there is one panel).
11. **Deep link `katto://dock`** (new `Route::Dock`) opens the dock panel; the
    needs-input notification uses it. `katto://ideas` keeps meaning the Planner surface.
12. **Curation sessions get permission allow-rules** in their generated settings
    (`Bash(sqlite3:*)`, plus discovery is run by katto itself, not the session); cut-plan
    sessions get `--permission-mode acceptEdits` so writing `cuts.json` doesn't stall. If
    a permission prompt still appears, the Notification hook → needs-input badge +
    notification is the honest, graceful behavior (owner decides in the morning).

## Design grounding (Dribbble pass, 2026-07-22)

Four searches ("terminal app ui", "command line terminal dark", "ai agent dashboard
sessions", "background jobs queue panel", "warp terminal"); three shots studied closely.

**Taken:** (a) dev-console pattern — the log/terminal region is a pure dark mono surface
while every surrounding control (tabs, filters, labels) is regular UI type on a lighter
chrome surface: exactly katto's mono-is-for-machines split; the xterm viewport is the only
mono thing in the dock. (b) Agent-activity-feed pattern — session rows read as
`state-dot + label + quiet muted subline` ("closed after idle · 2:14 PM"), not as loud
cards. (c) Async-jobs-queue popover — per-row dismiss (×) and a quiet footer link; the tab
strip gets per-tab close and the panel footer links "Open events log".

**Rejected:** neon purple/blue terminal palettes (katto stays warm-neutral + ember);
fake-window chrome (traffic-light dots) around the terminal — the dock is an in-app
slide-over, not a pretend window; right-side facet rails (overkill for ≤ a handful of
sessions); glass/translucent panels (banned); payment-terminal/mobile-app aesthetics.

## File map

New backend: `src-tauri/src/sessions.rs` + `sessions/{state,buffer,launch,hooks_endpoint,pool,reap}.rs`,
`src-tauri/src/scheduler.rs` + `scheduler/due.rs`, `src-tauri/src/curation.rs`,
`src-tauri/src/vfx.rs`, `src-tauri/src/commands/{sessions,scheduler,vfx}.rs`,
`src-tauri/src/db/{scheduled_jobs,raw_signal}.rs`,
`src-tauri/src/db/migrations/004_seed_nightly_curation.sql`.
Modified backend: `lib.rs` (state, handlers, setup), `state.rs`, `broadcast.rs`,
`notify.rs` (+ delegate), `commands/settings.rs`, `commands/pipeline.rs`,
`jobs/pipeline.rs`, `db.rs` (migrations list via `db/migrations.rs`), `Cargo.toml`.
New frontend: `src/features/dock/{dock-panel,tab-strip,terminal,dock-icon}.tsx`,
`src/features/dock/model/dock-state.ts`, `src/features/dock/use-sessions.ts`,
`src/features/vfx/{vfx-card,new-effect-dialog}.tsx`, `src/lib/ipc/{sessions,scheduler,vfx}.ts`,
`src/features/settings/components/dock-section.tsx`.
Modified frontend: `src/stores/ui.ts`, `src/components/layout/sidebar.tsx`,
`src/app/{app,commands}.ts(x)`, `src/hooks/{use-broadcast-invalidation,use-deep-link-router}.ts`,
`src/lib/ipc/broadcast.ts`, `src/styles/main.css` (stash procedure),
`src/features/projects/detail/project-detail.tsx` (stash procedure),
`src/features/planner/backlog/backlog-view.tsx`, `src/features/settings/settings-page.tsx`,
`package.json`.

---

### Task 1: Dependencies, migration 004, scheduled_jobs + raw_signal repos

**Files:**
- Modify: `src-tauri/Cargo.toml` (portable-pty, tiny_http; objc2-app-kit `NSWorkspace` feature)
- Create: `src-tauri/src/db/migrations/004_seed_nightly_curation.sql`
- Modify: `src-tauri/src/db/migrations.rs`
- Create: `src-tauri/src/db/scheduled_jobs.rs` (+ tests in-module)
- Create: `src-tauri/src/db/raw_signal.rs` (+ tests in-module)
- Modify: `src-tauri/src/db.rs` (declare new repo modules)
- Modify: `package.json` via bun (xterm packages)

**Interfaces:**
- Consumes: `crate::db::test_db()` (`src-tauri/src/db.rs:72`), `crate::error::Result`,
  `crate::db::RowId` pattern.
- Produces:
  - `db::scheduled_jobs::ScheduledJob { pub name: String, pub spec: String, pub last_success_at: Option<String>, pub enabled: bool }` (`Serialize, specta::Type, Clone, Debug`)
  - `pub fn list(conn: &Connection) -> Result<Vec<ScheduledJob>>`
  - `pub fn get(conn: &Connection, name: &str) -> Result<Option<ScheduledJob>>`
  - `pub fn set_last_success(conn: &Connection, name: &str, iso_utc: &str) -> Result<()>`
  - `pub fn update(conn: &Connection, name: &str, spec: &str, enabled: bool) -> Result<()>`
  - `db::raw_signal::pub fn judged_counts_since(conn: &Connection, iso_utc: &str) -> Result<(u32, u32)>` (kept, discarded)
  - `db::raw_signal::pub fn unjudged_count(conn: &Connection) -> Result<u32>`
  - `db::raw_signal::pub fn prune_judged_older_than_days(conn: &Connection, days: u32) -> Result<usize>`
  - `db::ideas` gains `pub fn count_since(conn: &Connection, iso_utc: &str) -> Result<u32>` (on `first_seen`)

- [ ] **Step 1: Invoke the `add-db-migration` skill**, then create the migration file.

`src-tauri/src/db/migrations/004_seed_nightly_curation.sql`:

```sql
INSERT OR IGNORE INTO scheduled_jobs (name, spec, last_success_at, enabled)
VALUES ('nightly-curation', 'daily@00:00;catchup=20h', NULL, 1);
```

Append to the vec in `src-tauri/src/db/migrations.rs`:

```rust
    M::up(include_str!("migrations/004_seed_nightly_curation.sql")),
```

- [ ] **Step 2: Write failing repo tests** (in-module `#[cfg(test)] mod tests`, names
  `<scenario>_<expected>`, no `test_` prefix). `src-tauri/src/db/scheduled_jobs.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_db;

    #[test]
    fn seed_row_lists_after_migrations() {
        let conn = test_db();
        let jobs = list(&conn).unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].name, "nightly-curation");
        assert_eq!(jobs[0].spec, "daily@00:00;catchup=20h");
        assert!(jobs[0].enabled);
        assert!(jobs[0].last_success_at.is_none());
    }

    #[test]
    fn set_last_success_round_trips() {
        let conn = test_db();
        set_last_success(&conn, "nightly-curation", "2026-07-22T08:00:00Z").unwrap();
        let job = get(&conn, "nightly-curation").unwrap().unwrap();
        assert_eq!(job.last_success_at.as_deref(), Some("2026-07-22T08:00:00Z"));
    }

    #[test]
    fn update_changes_spec_and_enabled() {
        let conn = test_db();
        update(&conn, "nightly-curation", "daily@02:30;catchup=20h", false).unwrap();
        let job = get(&conn, "nightly-curation").unwrap().unwrap();
        assert_eq!(job.spec, "daily@02:30;catchup=20h");
        assert!(!job.enabled);
    }

    #[test]
    fn get_unknown_name_is_none() {
        let conn = test_db();
        assert!(get(&conn, "nope").unwrap().is_none());
    }
}
```

And `src-tauri/src/db/raw_signal.rs` tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_db;

    fn seed_row(conn: &rusqlite::Connection, id: &str, judged_at: Option<&str>, verdict: Option<&str>, fetched_at: &str) {
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
        seed_row(&conn, "a", Some("2026-07-22 08:01:00"), Some("kept"), "2026-07-20 00:00:00");
        seed_row(&conn, "b", Some("2026-07-22 08:02:00"), Some("discarded"), "2026-07-20 00:00:00");
        seed_row(&conn, "c", Some("2026-07-21 00:00:00"), Some("discarded"), "2026-07-20 00:00:00");
        seed_row(&conn, "d", None, None, "2026-07-20 00:00:00");
        let (kept, discarded) = judged_counts_since(&conn, "2026-07-22 08:00:00").unwrap();
        assert_eq!((kept, discarded), (1, 1));
    }

    #[test]
    fn unjudged_count_counts_null_judged_at() {
        let conn = test_db();
        seed_row(&conn, "a", None, None, "2026-07-20 00:00:00");
        seed_row(&conn, "b", Some("2026-07-21 00:00:00"), Some("kept"), "2026-07-20 00:00:00");
        assert_eq!(unjudged_count(&conn).unwrap(), 1);
    }

    #[test]
    fn prune_deletes_only_old_judged_rows() {
        let conn = test_db();
        seed_row(&conn, "old-judged", Some("2026-01-01 00:00:00"), Some("discarded"), "2026-01-01 00:00:00");
        seed_row(&conn, "old-unjudged", None, None, "2026-01-01 00:00:00");
        seed_row(&conn, "new-judged", Some("2026-07-21 00:00:00"), Some("kept"), "2026-07-21 00:00:00");
        let n = prune_judged_older_than_days(&conn, 90).unwrap();
        assert_eq!(n, 1);
        assert_eq!(unjudged_count(&conn).unwrap(), 1);
    }
}
```

Add to `db/ideas.rs` tests:

```rust
    #[test]
    fn count_since_counts_first_seen_after_cutoff() {
        let conn = test_db();
        // use the existing create() helper from this repo's tests to insert two ideas,
        // then UPDATE ideas SET first_seen so one is before and one after the cutoff.
        // assert_eq!(count_since(&conn, "2026-07-22 08:00:00").unwrap(), 1);
    }
```

(Write it concretely against `db/ideas.rs`'s existing `create` signature — read that file
first; its tests already construct ideas.)

- [ ] **Step 3: Run tests to verify failure** —
  `cargo test -p katto scheduled_jobs` → compile error (module missing). Expected.

- [ ] **Step 4: Implement.** `src-tauri/src/db/scheduled_jobs.rs`:

```rust
use rusqlite::Connection;
use serde::Serialize;

use crate::error::Result;

/// A named recurring job with anacron-style catch-up semantics (Phase 6).
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ScheduledJob {
    pub name: String,
    pub spec: String,
    pub last_success_at: Option<String>,
    pub enabled: bool,
}

fn row_to_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScheduledJob> {
    Ok(ScheduledJob {
        name: row.get(0)?,
        spec: row.get(1)?,
        last_success_at: row.get(2)?,
        enabled: row.get::<_, i64>(3)? != 0,
    })
}

pub fn list(conn: &Connection) -> Result<Vec<ScheduledJob>> {
    let mut stmt =
        conn.prepare("SELECT name, spec, last_success_at, enabled FROM scheduled_jobs ORDER BY name")?;
    let jobs = stmt.query_map([], row_to_job)?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(jobs)
}

pub fn get(conn: &Connection, name: &str) -> Result<Option<ScheduledJob>> {
    let mut stmt =
        conn.prepare("SELECT name, spec, last_success_at, enabled FROM scheduled_jobs WHERE name = ?1")?;
    let mut rows = stmt.query_map([name], row_to_job)?;
    Ok(rows.next().transpose()?)
}

pub fn set_last_success(conn: &Connection, name: &str, iso_utc: &str) -> Result<()> {
    conn.execute("UPDATE scheduled_jobs SET last_success_at = ?2 WHERE name = ?1", (name, iso_utc))?;
    Ok(())
}

pub fn update(conn: &Connection, name: &str, spec: &str, enabled: bool) -> Result<()> {
    conn.execute(
        "UPDATE scheduled_jobs SET spec = ?2, enabled = ?3 WHERE name = ?1",
        (name, spec, enabled as i64),
    )?;
    Ok(())
}
```

`src-tauri/src/db/raw_signal.rs`:

```rust
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
    let counts = stmt.query_row([iso_utc], |row| Ok((row.get::<_, u32>(0)?, row.get::<_, u32>(1)?)))?;
    Ok(counts)
}

pub fn unjudged_count(conn: &Connection) -> Result<u32> {
    let n = conn.query_row("SELECT COUNT(*) FROM raw_signal WHERE judged_at IS NULL", [], |r| {
        r.get::<_, u32>(0)
    })?;
    Ok(n)
}

/// Housekeeping (PRD): judged rows older than `days` are deleted; unjudged rows never.
pub fn prune_judged_older_than_days(conn: &Connection, days: u32) -> Result<usize> {
    let n = conn.execute(
        "DELETE FROM raw_signal
         WHERE judged_at IS NOT NULL AND fetched_at < datetime('now', '-' || ?1 || ' days')",
        [days],
    )?;
    Ok(n)
}
```

Add `count_since` to `db/ideas.rs` (same `SELECT COUNT(*) FROM ideas WHERE first_seen >= ?1`
shape). Declare `pub mod scheduled_jobs; pub mod raw_signal;` where db.rs declares the
other repo modules.

- [ ] **Step 5: Add dependencies.**

`src-tauri/Cargo.toml`: add under `[dependencies]`:

```toml
portable-pty = "0.9"
tiny_http = "0.12"
```

and extend the existing macOS-target objc2-app-kit features:

```toml
objc2-app-kit = { version = "0.3", default-features = false, features = ["std", "NSWindow", "NSResponder", "NSWorkspace"] }
```

Frontend: `bun add @xterm/xterm@6.0.0 @xterm/addon-fit@0.11.0 @xterm/addon-webgl@0.19.0`.

- [ ] **Step 6: Run** `cargo test -p katto db::` — all new tests pass; the mandatory
  fresh-DB migration test still passes (004 applies cleanly).

- [ ] **Step 7: Commit via `ship`** — paths:
  `src-tauri/Cargo.toml Cargo.lock src-tauri/src/db/migrations/004_seed_nightly_curation.sql src-tauri/src/db/migrations.rs src-tauri/src/db/scheduled_jobs.rs src-tauri/src/db/raw_signal.rs src-tauri/src/db.rs src-tauri/src/db/ideas.rs package.json bun.lock`
  — `feat(db): seed nightly-curation schedule; scheduled_jobs + raw_signal repos`.

### Task 2: Session state machine (pure)

**Files:**
- Create: `src-tauri/src/sessions.rs` (module root: `pub mod state;` for now)
- Create: `src-tauri/src/sessions/state.rs`
- Modify: `src-tauri/src/lib.rs` (declare `mod sessions;`)

**Interfaces:**
- Produces (consumed by pool, reaper, commands, specta):
  - `pub enum SessionState { Running, NeedsInput, Idle, Failed { error: String }, Closed { reason: CloseReason } }` (`Clone, Debug, PartialEq, Serialize, specta::Type`, serde `tag = "kind"`, `rename_all = "snake_case"`)
  - `pub enum CloseReason { Exited, IdleReaped, UserClosed }` (same derives, snake_case)
  - `pub enum SessionEvent { HookStop, HookNotification, UserInput, OutputBytes { len: usize }, PtyExited { code: Option<u32> }, SilenceTimeout }` (`Clone, Debug, PartialEq`)
  - `pub fn apply(state: &SessionState, event: &SessionEvent, hooks_live: bool) -> SessionState`

Semantics (the test table IS the spec):
- Hook-driven (hooks_live = true): `Running --HookStop--> Idle`;
  `Running --HookNotification--> NeedsInput`; `NeedsInput --HookStop--> Idle`;
  `{Idle, NeedsInput} --UserInput--> Running`; `OutputBytes` never changes state
  (Claude's TUI repaints are not activity); `SilenceTimeout` is ignored.
- Degraded (hooks_live = false): `Running --SilenceTimeout--> Idle` (silence > 45 s,
  timer owned by the pool); `Idle --OutputBytes{len > 512}--> Running` (a real burst,
  not a repaint); `Idle --OutputBytes{len <= 512}--> Idle`; `UserInput --> Running`
  from Idle/NeedsInput as above; hooks events still honored if they ever arrive.
- Universally: any live state `--PtyExited{Some(0)}--> Closed{Exited}`;
  `--PtyExited{non-zero or None}--> Failed{error: "exited with status <code>"}`
  (Failed keeps its tab + scrollback, D18). `Failed`/`Closed` are terminal: every
  event maps to the same state (no resurrection).

- [ ] **Step 1: Write the failing table-driven test** (rstest is not currently a
  src-tauri dev-dep — check `src-tauri/Cargo.toml`; if absent use a plain
  `for` over a `Vec` of cases, which the testing rules allow for small tables, or add
  `rstest = "0.23"` to dev-dependencies to match engine style). In
  `src-tauri/src/sessions/state.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn cases() -> Vec<(&'static str, SessionState, SessionEvent, bool, SessionState)> {
        use CloseReason::*;
        use SessionEvent::*;
        use SessionState::*;
        vec![
            ("stop idles", Running, HookStop, true, Idle),
            ("notification needs input", Running, HookNotification, true, NeedsInput),
            ("stop clears needs-input", NeedsInput, HookStop, true, Idle),
            ("input resumes from idle", Idle, UserInput, true, Running),
            ("input resumes from needs-input", NeedsInput, UserInput, true, Running),
            ("output never changes state when hooks live", Idle, OutputBytes { len: 9000 }, true, Idle),
            ("silence ignored when hooks live", Running, SilenceTimeout, true, Running),
            ("degraded silence idles", Running, SilenceTimeout, false, Idle),
            ("degraded burst resumes", Idle, OutputBytes { len: 513 }, false, Running),
            ("degraded repaint stays idle", Idle, OutputBytes { len: 512 }, false, Idle),
            ("clean exit closes", Running, PtyExited { code: Some(0) }, true, Closed { reason: Exited }),
            ("dirty exit fails", Idle, PtyExited { code: Some(1) }, true,
                Failed { error: "exited with status 1".into() }),
            ("signal death fails", Running, PtyExited { code: None }, true,
                Failed { error: "exited with status unknown".into() }),
            ("failed is terminal", Failed { error: "x".into() }, HookStop, true, Failed { error: "x".into() }),
            ("closed is terminal", Closed { reason: UserClosed }, UserInput, true, Closed { reason: UserClosed }),
        ]
    }

    #[test]
    fn transition_table_holds() {
        for (name, from, event, hooks_live, want) in cases() {
            let got = apply(&from, &event, hooks_live);
            assert_eq!(got, want, "case: {name}");
        }
    }
}
```

- [ ] **Step 2: Run to verify failure** —
  `cargo test -p katto sessions::state` → compile error. Expected.

- [ ] **Step 3: Implement** `apply` as a single `match` over
  `(state, event, hooks_live)`; keep it total (no `unreachable!`). Derive
  `Serialize`/`specta::Type` with `#[serde(tag = "kind", rename_all = "snake_case")]`
  on `SessionState` and plain snake_case variants on `CloseReason` so the frontend
  discriminates on `state.kind`.

- [ ] **Step 4: Run** `cargo test -p katto sessions::state` — PASS.

- [ ] **Step 5: Commit via `ship`** — paths:
  `src-tauri/src/sessions.rs src-tauri/src/sessions/state.rs src-tauri/src/lib.rs`
  (+ `src-tauri/Cargo.toml Cargo.lock` if rstest added) —
  `feat(sessions): pure session state machine with hook/degraded transitions`.

---

### Task 3: Scrollback ring buffer (pure)

**Files:**
- Create: `src-tauri/src/sessions/buffer.rs`
- Modify: `src-tauri/src/sessions.rs` (`pub mod buffer;`)

**Interfaces:**
- Produces: `pub struct Scrollback { ... }` with
  `pub fn new(cap: usize) -> Self`, `pub fn push(&mut self, bytes: &[u8])`,
  `pub fn snapshot(&self) -> Vec<u8>`, `pub fn len(&self) -> usize`,
  `pub fn is_empty(&self) -> bool`.
  Pool uses `Scrollback::new(2 * 1024 * 1024)`.

- [ ] **Step 1: Failing tests** in-module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_returns_pushed_bytes_in_order() {
        let mut sb = Scrollback::new(16);
        sb.push(b"hello ");
        sb.push(b"world");
        assert_eq!(sb.snapshot(), b"hello world");
    }

    #[test]
    fn overflow_drops_oldest_bytes() {
        let mut sb = Scrollback::new(8);
        sb.push(b"abcdefgh");
        sb.push(b"XY");
        assert_eq!(sb.snapshot(), b"cdefghXY");
        assert_eq!(sb.len(), 8);
    }

    #[test]
    fn push_larger_than_cap_keeps_tail() {
        let mut sb = Scrollback::new(4);
        sb.push(b"0123456789");
        assert_eq!(sb.snapshot(), b"6789");
    }

    #[test]
    fn empty_snapshot_is_empty() {
        let sb = Scrollback::new(4);
        assert!(sb.is_empty());
        assert_eq!(sb.snapshot(), Vec::<u8>::new());
    }
}
```

- [ ] **Step 2: Run to fail** — `cargo test -p katto sessions::buffer`. Expected: compile error.

- [ ] **Step 3: Implement** over `std::collections::VecDeque<u8>` (`push` extends then
  drains front down to cap; `snapshot` copies contiguous via `iter().copied().collect()`).
  Keep it allocation-simple — this is 2 MiB max, not a hot path.

- [ ] **Step 4: Run** — PASS.

- [ ] **Step 5: Commit via `ship`** — paths:
  `src-tauri/src/sessions/buffer.rs src-tauri/src/sessions.rs` —
  `feat(sessions): bounded scrollback ring buffer`.

---

### Task 4: Launch plumbing — hook settings JSON + spawn command (pure)

**Files:**
- Create: `src-tauri/src/sessions/launch.rs`
- Modify: `src-tauri/src/sessions.rs` (`pub mod launch;`)

**Interfaces:**
- Produces (consumed by pool Task 6, curation Task 14, cut re-route Task 15):
  - `pub struct LaunchSpec { pub claude_path: PathBuf, pub cwd: PathBuf, pub initial_prompt: Option<String>, pub append_system_prompt: Option<String>, pub settings_path: PathBuf, pub permission_mode: Option<String> }`
  - `pub fn hook_settings_json(endpoint_port: u16, token: &str, session_id: &str, permission_allow: &[String]) -> String`
    — serde_json built (never string-formatted), shape:
    `{"hooks": {"Stop": [...curl...], "Notification": [{"matcher": "permission_prompt|idle_prompt", "hooks": [...curl...]}]}, "permissions": {"allow": [...]}}`
    where each curl is a shell-form command hook:
    `curl -s -m 3 -X POST -H 'x-katto-token: <token>' --data-binary @- http://127.0.0.1:<port>/hook/<session_id> || true`
    (`permissions` key omitted when `permission_allow` is empty).
  - `pub fn shell_invocation(spec: &LaunchSpec) -> String` — the single string passed to
    `zsh -lc`: `exec <claude> --settings <file> [--permission-mode <m>] [--append-system-prompt <quoted>] [<quoted initial prompt>]`,
    with every operand single-quote shell-escaped via
    `pub fn sh_quote(s: &str) -> String` (wrap in `'…'`, embedded `'` → `'\''`).

- [ ] **Step 1: Failing tests:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn sh_quote_wraps_and_escapes_single_quotes() {
        assert_eq!(sh_quote("plain"), "'plain'");
        assert_eq!(sh_quote("it's"), r#"'it'\''s'"#);
        assert_eq!(sh_quote("a b; rm -rf /"), "'a b; rm -rf /'");
    }

    #[test]
    fn hook_settings_json_carries_both_hooks_and_token() {
        let json = hook_settings_json(43111, "tok-abc", "sess-1", &[]);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let stop_cmd = v["hooks"]["Stop"][0]["hooks"][0]["command"].as_str().unwrap();
        assert!(stop_cmd.contains("http://127.0.0.1:43111/hook/sess-1"));
        assert!(stop_cmd.contains("x-katto-token: tok-abc"));
        assert!(stop_cmd.ends_with("|| true"));
        let notif = &v["hooks"]["Notification"][0];
        assert_eq!(notif["matcher"], "permission_prompt|idle_prompt");
        assert!(v.get("permissions").is_none());
    }

    #[test]
    fn hook_settings_json_includes_allow_rules_when_given() {
        let json = hook_settings_json(1, "t", "s", &["Bash(sqlite3:*)".to_string()]);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["permissions"]["allow"][0], "Bash(sqlite3:*)");
    }

    #[test]
    fn shell_invocation_orders_flags_and_quotes_prompt() {
        let spec = LaunchSpec {
            claude_path: PathBuf::from("/usr/local/bin/claude"),
            cwd: PathBuf::from("/tmp"),
            initial_prompt: Some("plan the cut; it's due".into()),
            append_system_prompt: Some("sys".into()),
            settings_path: PathBuf::from("/data/sessions/s1.settings.json"),
            permission_mode: Some("acceptEdits".into()),
        };
        let line = shell_invocation(&spec);
        assert!(line.starts_with("exec '/usr/local/bin/claude' --settings '/data/sessions/s1.settings.json'"));
        assert!(line.contains("--permission-mode 'acceptEdits'"));
        assert!(line.contains("--append-system-prompt 'sys'"));
        assert!(line.ends_with(r#"'plan the cut; it'\''s due'"#));
    }

    #[test]
    fn shell_invocation_minimal_has_no_optional_flags() {
        let spec = LaunchSpec {
            claude_path: PathBuf::from("/x/claude"),
            cwd: PathBuf::from("/tmp"),
            initial_prompt: None,
            append_system_prompt: None,
            settings_path: PathBuf::from("/d/s.json"),
            permission_mode: None,
        };
        let line = shell_invocation(&spec);
        assert_eq!(line, "exec '/x/claude' --settings '/d/s.json'");
    }
}
```

- [ ] **Step 2: Run to fail** — `cargo test -p katto sessions::launch`.

- [ ] **Step 3: Implement.** Build the JSON with `serde_json::json!` and
  `serde_json::to_string_pretty`; build the curl string with `format!` (the token and
  port are katto-generated hex/uuid + u16 — no shell metacharacters by construction, but
  still pass the URL/header as literal text, no interpolated user input). Note in a doc
  comment why shell-form: the hook must pipe its stdin JSON straight to curl.

- [ ] **Step 4: Run** — PASS. Also `cargo clippy -p katto` clean.

- [ ] **Step 5: Commit via `ship`** — paths:
  `src-tauri/src/sessions/launch.rs src-tauri/src/sessions.rs` —
  `feat(sessions): hook-settings JSON and zsh invocation builders`.

### Task 5: Hooks endpoint (tiny_http)

**Files:**
- Create: `src-tauri/src/sessions/hooks_endpoint.rs`
- Modify: `src-tauri/src/sessions.rs` (`pub mod hooks_endpoint;`)

**Interfaces:**
- Produces:
  - `pub struct HooksEndpoint { pub port: u16, pub token: String }` (plus a private
    shutdown flag + join handle)
  - `pub enum HookEvent { Stop { session_id: String }, Notification { session_id: String } }` (`Debug, PartialEq`)
  - `pub fn start(on_event: std::sync::mpsc::Sender<HookEvent>) -> crate::error::Result<HooksEndpoint>`
    — binds `127.0.0.1:0`, generates a `uuid::Uuid::new_v4()` token, spawns the accept
    thread (`recv_timeout(250ms)` loop checking an `Arc<AtomicBool>` stop flag).
  - `impl HooksEndpoint { pub fn shutdown(&self) }`
- Request contract: `POST /hook/<session_id>` with header `x-katto-token: <token>`; body
  is Claude's hook stdin JSON — only `hook_event_name` is read (`"Stop"` /
  `"Notification"`); unknown names are ignored (200). Bad/missing token → 401, no event.
  Non-POST or unknown path → 404. Body read is capped (64 KB) — ignore the rest.

- [ ] **Step 1: Failing test** — a real bind + raw HTTP over `std::net::TcpStream`
  (no client dep needed):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    fn post(port: u16, path: &str, token: Option<&str>, body: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let token_header = token.map(|t| format!("x-katto-token: {t}\r\n")).unwrap_or_default();
        write!(
            stream,
            "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n{token_header}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn stop_hook_with_token_dispatches_event() {
        let (tx, rx) = std::sync::mpsc::channel();
        let ep = start(tx).unwrap();
        let resp = post(ep.port, "/hook/sess-9", Some(&ep.token), r#"{"hook_event_name":"Stop"}"#);
        assert!(resp.starts_with("HTTP/1.1 200"));
        let event = rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
        assert_eq!(event, HookEvent::Stop { session_id: "sess-9".into() });
        ep.shutdown();
    }

    #[test]
    fn bad_token_is_rejected_without_event() {
        let (tx, rx) = std::sync::mpsc::channel();
        let ep = start(tx).unwrap();
        let resp = post(ep.port, "/hook/sess-9", Some("wrong"), r#"{"hook_event_name":"Stop"}"#);
        assert!(resp.starts_with("HTTP/1.1 401"));
        assert!(rx.recv_timeout(std::time::Duration::from_millis(300)).is_err());
        ep.shutdown();
    }

    #[test]
    fn notification_hook_maps_to_needs_input_event() {
        let (tx, rx) = std::sync::mpsc::channel();
        let ep = start(tx).unwrap();
        post(ep.port, "/hook/s1", Some(&ep.token), r#"{"hook_event_name":"Notification","message":"perm"}"#);
        assert_eq!(
            rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap(),
            HookEvent::Notification { session_id: "s1".into() }
        );
        ep.shutdown();
    }
}
```

- [ ] **Step 2: Run to fail** — `cargo test -p katto hooks_endpoint`.

- [ ] **Step 3: Implement.** `tiny_http::Server::http("127.0.0.1:0")`; extract the port
  from `server.server_addr()`; thread loop:
  `while !stop.load(Ordering::Relaxed) { if let Ok(Some(mut req)) = server.recv_timeout(250ms) { … } }`.
  Parse path with `req.url().strip_prefix("/hook/")`; compare token from
  `req.headers().iter().find(|h| h.field.equiv("x-katto-token"))`. Parse body via
  `serde_json::from_slice::<serde_json::Value>` on a capped read; read
  `value["hook_event_name"].as_str()`. Respond `Response::empty(200|401|404)` — errors
  respond, never panic; a send failure on `on_event` (receiver gone) just logs via an
  events row? No — the endpoint has no DB handle; it simply stops dispatching (the pool
  owns lifecycle). Never log request bodies (may contain transcript paths — fine — but
  keep the no-noise rule: log nothing).

- [ ] **Step 4: Run** — PASS.

- [ ] **Step 5: Commit via `ship`** — paths:
  `src-tauri/src/sessions/hooks_endpoint.rs src-tauri/src/sessions.rs` —
  `feat(sessions): token-authed localhost hooks endpoint`.

---

### Task 6: PTY spawn + session pool (+ jobs-row integration)

The heart of the phase. Everything impure lives here, kept thin; the tested surface is
the pool's observable behavior against a fake shell.

**Files:**
- Create: `src-tauri/src/sessions/pty.rs`
- Create: `src-tauri/src/sessions/pool.rs`
- Modify: `src-tauri/src/sessions.rs` (`pub mod pty; pub mod pool;` + shared types)
- Modify: `src-tauri/src/state.rs` (add pool to `AppState`)
- Modify: `src-tauri/src/lib.rs` (construct pool + endpoint in `bootstrap_state`/setup)
- Modify: `src-tauri/src/error.rs` (new variants)

**Interfaces:**
- Consumes: Tasks 2–5 (`SessionState`, `apply`, `Scrollback`, `LaunchSpec`,
  `hook_settings_json`, `shell_invocation`, `HooksEndpoint`, `HookEvent`),
  `JobRuntime::spawn(kind, label, payload_json, work)` (`src-tauri/src/jobs.rs:77` —
  `work: FnOnce(JobContext) -> Future<Output = Result<(), String>>`),
  `db::events::record`, `broadcast` helpers (Task 8 adds the new events — until then the
  pool takes an `AppHandle` and calls stubs added here),
  `katto_engine::detect::detect_claude() -> Option<PathBuf>`.
- Produces:
  - `sessions::SessionTask { pub label: String, pub cwd: PathBuf, pub initial_prompt: Option<String>, pub append_system_prompt: Option<String>, pub permission_mode: Option<String>, pub permission_allow: Vec<String> }`
  - `sessions::SessionInfo { pub id: String, pub label: String, pub state: SessionState, pub cwd: String, pub started_at: String, pub idle_since_secs: Option<u64> }` (`Serialize, specta::Type, Clone`)
  - `pool::SessionPool` (`Clone` via inner `Arc`):
    - `pub fn new() -> Self` (endpoint started lazily on first spawn, or eagerly in setup — eagerly: `pub fn start(&self, app: AppHandle) -> Result<()>` wires the endpoint + the hook-event pump thread)
    - `pub async fn spawn(&self, app: &AppHandle, task: SessionTask, program: Program) -> Result<String>` where `pub enum Program { Claude, Custom(String) }` (`Custom` exists for tests: `bash -c 'cat'`)
    - `pub fn write(&self, id: &str, data: &[u8]) -> Result<()>` (marks `UserInput`)
    - `pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<()>`
    - `pub fn attach(&self, id: &str, channel: tauri::ipc::Channel<Vec<u8>>) -> Result<()>` (replays scrollback as first send, then live)
    - `pub async fn close(&self, id: &str, reason: CloseReason) -> Result<()>` (kill child, settings file cleanup, state → Closed, job resolution)
    - `pub fn list(&self) -> Vec<SessionInfo>`
    - `pub fn set_dock_focus(&self, open: bool, focused: Option<String>)` (reap exemption + needs-input notification suppression)
    - `pub fn on_state_change(&self, id, new_state)` internal: emits broadcast, events row, needs-input notification when panel hidden.
  - `pty.rs`: `pub struct PtyHandle { pub writer: Box<dyn Write + Send>, pub master: Box<dyn MasterPty + Send>, pub killer: Box<dyn ChildKiller + Send + Sync>, pub reader: Box<dyn Read + Send>, pub child: Box<dyn Child + Send> }` and
    `pub fn spawn_pty(program: &str, args: &[&str], cwd: &Path, cols: u16, rows: u16) -> Result<PtyHandle>`
    — the ONLY portable-pty call site: `native_pty_system().openpty(PtySize{rows, cols, pixel_width: 0, pixel_height: 0})`,
    `CommandBuilder::new(program)` + args + `.cwd(cwd)` + `.env("TERM", "xterm-256color")`,
    `slave.spawn_command`, then drop the slave. Untested-by-unit (thin), covered by the
    pool integration test.

Pool internals (document in code, no test on private layout):
`Arc<Inner { sessions: Mutex<HashMap<String, Entry>>, endpoint: OnceLock<HooksEndpoint>, dock: Mutex<DockFocus>, app: OnceLock<AppHandle> }>`.
`Entry { label, cwd, state: SessionState, hooks_live: bool, buffer: Scrollback, writer, killer, sink: Option<Channel<Vec<u8>>>, started_at: String, idle_since: Option<Instant>, last_output: Instant, job_done: Option<tokio::sync::oneshot::Sender<std::result::Result<(), String>>>, settings_path: Option<PathBuf> }`.
Threads per session: reader (blocking `read` → `mpsc::Sender<Vec<u8>>`), flusher
(`recv_timeout(16ms)`, flush at ≥16 KB or timeout with pending, into
`pool.ingest_output(id, bytes)` which pushes scrollback + sends to sink + feeds
`OutputBytes` through `apply` when degraded), waiter (blocking `child.wait()` →
`pool.on_exit(id, status)`). Hook pump thread: drains the endpoint's
`mpsc::Receiver<HookEvent>`, sets `hooks_live = true` for that session, applies
`HookStop`/`HookNotification`. A 15 s tokio interval (started in `start`) applies
`SilenceTimeout` to hooks-degraded Running sessions silent > 45 s and, on the FIRST
degraded transition per session, records events row `session_hooks_degraded`.

Jobs-row integration: `spawn` calls
`state.jobs.spawn("claude_session", &task.label, Some(json!({"session_id": id}).to_string()), move |_ctx| async move { done_rx.await.unwrap_or(Err("session dropped".into())) })`
— the oneshot `done_tx` lives in the Entry; `close`/`on_exit` resolve it
(`Ok(())` for Exited/IdleReaped/UserClosed, `Err(msg)` for Failed). The existing jobs
framework then writes `job_done`/`job_failed` events rows and broadcasts — sessions ride
the standard machinery. Additional session-specific events rows: `session_spawned`
(payload `{session_id, label, cwd}`) on spawn, `session_reaped` on idle reap.

Spawn flow (Program::Claude): resolve claude path (settings `claude_path` else
`detect_claude()`, else `Error::NoPlanner`-style new variant `Error::ClaudeMissing` →
frontend shows install hint per PRD); ensure `<app_data_dir>/sessions/` exists; write
`<id>.settings.json` with `hook_settings_json(port, token, id, &task.permission_allow)`
(perms 0o600 via `std::os::unix::fs::PermissionsExt`); build `LaunchSpec` →
`spawn_pty("zsh", &["-lc", &shell_invocation(&spec)], &task.cwd, 120, 30)`; insert Entry
with state Running; start threads; events row; broadcast.

- [ ] **Step 1: Failing integration test** (in `pool.rs` `#[cfg(test)]`; these use
  `Program::Custom` with plain shells — no claude, no tauri app: the pool's
  app-dependent side effects (jobs row, events, broadcast, notification) must be behind
  small `fn`s that no-op when `app` is unset, so the pool is testable headless. Note the
  test constructs the pool WITHOUT `start(app)` — endpoint still starts, hook pump still
  runs):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn task(label: &str) -> SessionTask {
        SessionTask {
            label: label.into(),
            cwd: std::env::temp_dir(),
            initial_prompt: None,
            append_system_prompt: None,
            permission_mode: None,
            permission_allow: vec![],
        }
    }

    #[tokio::test]
    async fn echo_shell_round_trips_and_batches() {
        let pool = SessionPool::new();
        let id = pool.spawn_headless(task("t"), Program::Custom("bash -c 'cat'".into())).await.unwrap();
        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        pool.attach_sink(&id, Box::new(move |bytes| tx.send(bytes.to_vec()).is_ok()));
        pool.write(&id, b"hello\n").unwrap();
        let mut got = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline && !String::from_utf8_lossy(&got).contains("hello") {
            if let Ok(chunk) = rx.recv_timeout(Duration::from_millis(100)) {
                got.extend(chunk);
            }
        }
        assert!(String::from_utf8_lossy(&got).contains("hello"));
        pool.close(&id, CloseReason::UserClosed).await.unwrap();
        assert!(matches!(pool.list()[0].state, SessionState::Closed { .. }));
    }

    #[tokio::test]
    async fn scrollback_replays_on_attach() {
        let pool = SessionPool::new();
        let id = pool.spawn_headless(task("t"), Program::Custom("bash -c 'echo pre-attach; cat'".into())).await.unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        pool.attach_sink(&id, Box::new(move |bytes| tx.send(bytes.to_vec()).is_ok()));
        let first = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(String::from_utf8_lossy(&first).contains("pre-attach"));
        pool.close(&id, CloseReason::UserClosed).await.unwrap();
    }

    #[tokio::test]
    async fn clean_exit_transitions_to_closed() {
        let pool = SessionPool::new();
        let id = pool.spawn_headless(task("t"), Program::Custom("bash -c 'exit 0'".into())).await.unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if matches!(pool.get_state(&id), Some(SessionState::Closed { .. })) { break; }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(matches!(pool.get_state(&id), Some(SessionState::Closed { reason: CloseReason::Exited })));
    }

    #[tokio::test]
    async fn dirty_exit_transitions_to_failed_and_keeps_scrollback() {
        let pool = SessionPool::new();
        let id = pool.spawn_headless(task("t"), Program::Custom("bash -c 'echo boom >&2; exit 3'".into())).await.unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if matches!(pool.get_state(&id), Some(SessionState::Failed { .. })) { break; }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        match pool.get_state(&id) {
            Some(SessionState::Failed { error }) => assert!(error.contains('3')),
            other => panic!("expected Failed, got {other:?}"),
        }
        assert!(!pool.scrollback(&id).unwrap().is_empty());
    }

    #[tokio::test]
    async fn resize_propagates_without_error() {
        let pool = SessionPool::new();
        let id = pool.spawn_headless(task("t"), Program::Custom("bash -c 'cat'".into())).await.unwrap();
        pool.resize(&id, 200, 50).unwrap();
        pool.close(&id, CloseReason::UserClosed).await.unwrap();
    }
}
```

Design note the test imposes: expose test-facing seams
`pub(crate) async fn spawn_headless(&self, task, program) -> Result<String>` (spawn minus
jobs/events/broadcast), `pub(crate) fn attach_sink(&self, id, sink: Box<dyn Fn(&[u8]) -> bool + Send>)`
(the `Channel` attach wraps this — `attach` builds a sink closure over
`channel.send(bytes.to_vec()).is_ok()`), `pub(crate) fn get_state(&self, id) -> Option<SessionState>`,
`pub(crate) fn scrollback(&self, id) -> Option<Vec<u8>>`. The public `attach` stays a
2-liner over `attach_sink`. tokio tests need `tokio = { features = ["rt", "macros"] }`
in dev-dependencies (`rt` already there; add `macros` if missing).

- [ ] **Step 2: Run to fail** — `cargo test -p katto sessions::pool`.

- [ ] **Step 3: Implement `pty.rs` then `pool.rs`** per the internals sketch above.
  Rules: no `unwrap()` outside tests; lock scope tight (never hold the sessions Mutex
  across `.await` — collect what you need, drop the guard, then await); the flusher owns
  batching (16 ms / 16 KB); `close` drops the writer + kills via `killer.kill()`, the
  waiter thread observes the exit — `close` must WIN the state race: set
  `Closed{reason}` before kill so `on_exit` only applies `PtyExited` to still-live
  states (the Task 2 machine already makes Closed terminal). Delete the settings file on
  close/exit (`let _ = std::fs::remove_file(...)`).

- [ ] **Step 4: Wire into `AppState`** — add `pub sessions: crate::sessions::pool::SessionPool`
  to `AppState` (`src-tauri/src/state.rs:11`); construct in `bootstrap_state`
  (`src-tauri/src/lib.rs:97`); call `state.sessions.start(handle.clone())` in the setup
  hook next to the other spawns. `Error` variants: `ClaudeMissing`,
  `SessionNotFound(String)`, `SessionSpawn(String)` with the existing serialize-tagged
  pattern in `error.rs`.

- [ ] **Step 5: Run** — `cargo test -p katto sessions::` PASS; `cargo clippy -p katto`
  clean.

- [ ] **Step 6: Real-claude smoke test (OPTIONAL, budget: 1 call).** Add
  `#[ignore = "spawns real claude; run manually"]` test `real_claude_session_smoke`
  that spawns `Program::Claude` equivalent via `spawn_headless` with a
  `LaunchSpec`-built invocation and initial prompt `"Reply with the single word pong, then wait."`,
  waits ≤ 60 s for `pong` in scrollback, then closes. Run once:
  `cargo test -p katto real_claude_session_smoke -- --ignored --nocapture`. If flaky
  in CI-like contexts, leave it `#[ignore]`d — that is its permanent state.

- [ ] **Step 7: Commit via `ship`** — paths:
  `src-tauri/src/sessions/pty.rs src-tauri/src/sessions/pool.rs src-tauri/src/sessions.rs src-tauri/src/state.rs src-tauri/src/lib.rs src-tauri/src/error.rs src-tauri/Cargo.toml Cargo.lock` —
  `feat(sessions): PTY-backed session pool with hook-driven state and jobs integration`.

---

### Task 7: Idle reaper

**Files:**
- Create: `src-tauri/src/sessions/reap.rs`
- Modify: `src-tauri/src/sessions.rs` (`pub mod reap;`)
- Modify: `src-tauri/src/sessions/pool.rs` (reaper tick wiring)
- Modify: `src-tauri/src/commands/settings.rs` (`DEFAULT_IDLE_REAP_MINUTES` 10 → 5)

**Interfaces:**
- Produces: `pub fn should_reap(state: &SessionState, idle_for: Option<Duration>, timeout: Duration, exempt: bool) -> bool`
- Consumes: settings key `idle_reap_minutes` (existing), `SessionPool::close(id, CloseReason::IdleReaped)`.

- [ ] **Step 1: Failing tests:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::state::{CloseReason, SessionState};
    use std::time::Duration;

    const FIVE_MIN: Duration = Duration::from_secs(300);

    #[test]
    fn idle_past_timeout_reaps() {
        assert!(should_reap(&SessionState::Idle, Some(Duration::from_secs(301)), FIVE_MIN, false));
    }

    #[test]
    fn idle_under_timeout_survives() {
        assert!(!should_reap(&SessionState::Idle, Some(Duration::from_secs(299)), FIVE_MIN, false));
    }

    #[test]
    fn focused_panel_exempts() {
        assert!(!should_reap(&SessionState::Idle, Some(Duration::from_secs(9999)), FIVE_MIN, true));
    }

    #[test]
    fn needs_input_never_reaped() {
        assert!(!should_reap(&SessionState::NeedsInput, Some(Duration::from_secs(9999)), FIVE_MIN, false));
    }

    #[test]
    fn running_failed_closed_never_reaped() {
        for s in [
            SessionState::Running,
            SessionState::Failed { error: "x".into() },
            SessionState::Closed { reason: CloseReason::Exited },
        ] {
            assert!(!should_reap(&s, Some(Duration::from_secs(9999)), FIVE_MIN, false));
        }
    }
}
```

- [ ] **Step 2: Run to fail**, **Step 3: implement** (a 4-line function), **Step 4: pass.**

- [ ] **Step 5: Wire the tick.** In `SessionPool::start`, spawn a
  `tauri::async_runtime::spawn` loop with `tokio::time::interval(Duration::from_secs(30))`:
  read `idle_reap_minutes` via `db::settings::get` (parse, default 5); for each session
  where `should_reap(...)` (exempt = dock focus says open AND focused on this id) call
  `close(id, CloseReason::IdleReaped)` — which records events row `session_reaped`
  payload `{session_id, label, idle_minutes}`. The tab-strip note ("closed after idle")
  falls out of `CloseReason::IdleReaped` in `SessionInfo`. Change
  `DEFAULT_IDLE_REAP_MINUTES` to `5` and fix its doc comment; adjust any settings test
  asserting 10.

- [ ] **Step 6: Run** `cargo test -p katto` — PASS (including the adjusted settings test).

- [ ] **Step 7: Commit via `ship`** — paths:
  `src-tauri/src/sessions/reap.rs src-tauri/src/sessions.rs src-tauri/src/sessions/pool.rs src-tauri/src/commands/settings.rs` —
  `feat(sessions): idle reaper with focus exemption; default 5 min`.

### Task 8: Session commands, broadcast events, needs-input notification, bindings

**Files:**
- Create: `src-tauri/src/commands/sessions.rs`
- Modify: `src-tauri/src/commands.rs` (declare module)
- Modify: `src-tauri/src/broadcast.rs` (new events)
- Modify: `src-tauri/src/lib.rs` (`collect_commands!` + `collect_events!`)
- Modify: `src-tauri/src/notify.rs` (`Route::Dock`)

**Interfaces:**
- Consumes: `SessionPool` (Task 6), `SessionTask`, `SessionInfo`, broadcast pattern
  (`src-tauri/src/broadcast.rs` — `#[derive(Serialize, Clone, specta::Type, tauri_specta::Event)]`).
- Produces (frontend contract, via bindings):
  - Commands (invoke the **`add-tauri-command` skill first**):
    `spawn_session(task: SessionTask) -> Result<String>`,
    `attach_session(id: String, on_data: Channel<Vec<u8>>) -> Result<()>`,
    `write_session(id: String, data: String) -> Result<()>` (UTF-8 keystrokes from
    xterm `onData` — pool writes `data.as_bytes()`),
    `resize_session(id: String, cols: u16, rows: u16) -> Result<()>`,
    `close_session(id: String) -> Result<()>` (reason UserClosed),
    `list_sessions() -> Result<Vec<SessionInfo>>`,
    `set_dock_focus(open: bool, focused_session: Option<String>) -> Result<()>`.
    All thin shells over the pool; `spawn_session` uses `Program::Claude`.
    Public `SessionTask` (specta) excludes `permission_mode`/`permission_allow` — the
    command fills defaults (`None`, `[]`); internal callers (curation, cut re-route)
    construct the full struct directly.
  - Broadcast events: `SessionsChanged` (unit, on spawn/close/reap) → helper
    `pub fn sessions_changed(app: &AppHandle)`; `SessionStateChanged { pub id: String, pub state: crate::sessions::state::SessionState }`
    → `pub fn session_state_changed(app: &AppHandle, id: &str, state: &SessionState)`.
  - `notify::Route::Dock` with `as_wire() -> "dock"` and `parse_deep_link("katto://dock")`.

- [ ] **Step 1: Failing tests.** `notify.rs` already has route tests — extend them:

```rust
    #[test]
    fn dock_route_round_trips() {
        assert_eq!(parse_deep_link("katto://dock"), Some(Route::Dock));
        assert_eq!(Route::Dock.as_wire(), "dock");
    }
```

- [ ] **Step 2: Run to fail** — `cargo test -p katto notify`.

- [ ] **Step 3: Implement** commands module (match the shape of
  `commands/jobs.rs` — thin, `State<'_, AppState>`, `Result<T, Error>`); broadcast
  structs + helpers copying the existing derive block style; `Route::Dock`. Register
  every command in `collect_commands!` and both events in `collect_events!`
  (`src-tauri/src/lib.rs:27/:81`).
  **Needs-input notification wiring** (pool `on_state_change`, Task 6 stub → real): on
  transition to `NeedsInput` when dock focus says the panel is hidden (or the WebView is
  destroyed), call
  `crate::notify::notify(app, "Claude needs input", &format!("{label} is waiting on you"), "katto://dock")`
  — plus events row `session_needs_input` `{session_id, label}`. Broadcast
  `session_state_changed` on EVERY transition and `sessions_changed` on
  spawn/close/reap.

- [ ] **Step 4: Regenerate bindings + full backend test** —
  `cargo test -p katto export_bindings` then `cargo test -p katto`. Expected: PASS,
  `src/lib/ipc/bindings.gen.ts` now carries `spawnSession…` + `SessionState` etc.

- [ ] **Step 5: Commit via `ship`** — paths:
  `src-tauri/src/commands/sessions.rs src-tauri/src/commands.rs src-tauri/src/broadcast.rs src-tauri/src/lib.rs src-tauri/src/notify.rs src-tauri/src/sessions/pool.rs src/lib/ipc/bindings.gen.ts` —
  `feat(sessions): IPC commands, state broadcasts, dock deep link, needs-input notification`.

---

### Task 9: Dock frontend core — tokens, store, IPC, tab strip, panel, sidebar icon

**Files:**
- Modify: `src/styles/main.css` (**STASH PROCEDURE** — Global Constraints)
- Modify: `src/stores/ui.ts`
- Create: `src/lib/ipc/sessions.ts`
- Create: `src/features/dock/use-sessions.ts`
- Create: `src/features/dock/model/dock-state.ts` (+ test)
- Create: `src/features/dock/tab-strip.tsx` (+ test)
- Create: `src/features/dock/dock-panel.tsx` (+ test)
- Create: `src/features/dock/dock-icon.tsx`
- Modify: `src/components/layout/sidebar.tsx` (activate the placeholder)
- Modify: `src/app/app.tsx` (mount panel overlay)
- Modify: `src/lib/ipc/broadcast.ts` + `src/hooks/use-broadcast-invalidation.ts`

Invoke the **`add-feature-surface` skill** before creating `src/features/dock/`.

**Interfaces:**
- Consumes: bindings from Task 8 (`commands.spawnSession`, `commands.listSessions`,
  `commands.writeSession`, `commands.resizeSession`, `commands.closeSession`,
  `commands.attachSession`, `commands.setDockFocus`; events `sessionsChanged`,
  `sessionStateChanged`; types `SessionInfo`, `SessionState`, `SessionTask`), `unwrap()`
  from `src/lib/ipc/result.ts`, `useUiStore` selector style.
- Produces:
  - `src/lib/ipc/sessions.ts`: `sessionsKeys = { all: ["sessions"] as const }`,
    `listSessions(): Promise<SessionInfo[]>`, `spawnSession(task: SessionTask)`,
    `writeSession(id, data)`, `resizeSession(id, cols, rows)`, `closeSession(id)`,
    `setDockFocus(open, focusedSession)`, `attachSession(id, onData: (bytes: Uint8Array) => void): Promise<void>`
    (constructs `new Channel<number[]>()`, `ch.onmessage = (d) => onData(new Uint8Array(d))`).
  - `useUiStore` additions: `dockOpen: boolean`, `activeSessionId: string | null`,
    `toggleDock()`, `openDock(sessionId?: string)`, `closeDock()`, `setActiveSession(id)`.
  - `model/dock-state.ts`:
    `export type DockIconState = "idle" | "running" | "needs-input";`
    `export function deriveDockIconState(sessions: SessionInfo[]): DockIconState`
    (any `needs_input` → `"needs-input"`; else any `running` → `"running"`; else `"idle"` —
    Failed/Closed/Idle don't animate) and
    `export function tabNote(s: SessionInfo): string | null`
    (`closed + idle_reaped` → `"closed after idle"`, `failed` → its error, else null).

- [ ] **Step 1: Failing model tests** `src/features/dock/model/dock-state.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { deriveDockIconState, tabNote } from "./dock-state";
import type { SessionInfo } from "@/lib/ipc/bindings.gen";

const base: Omit<SessionInfo, "state"> = {
  id: "s1", label: "cut plan: a.mp4", cwd: "/x", startedAt: "2026-07-22 08:00:00", idleSinceSecs: null,
};
const with_ = (state: SessionInfo["state"], id = "s1"): SessionInfo => ({ ...base, id, state });

describe("deriveDockIconState", () => {
  it("prefers needs-input over running", () => {
    expect(
      deriveDockIconState([with_({ kind: "running" }, "a"), with_({ kind: "needs_input" }, "b")]),
    ).toBe("needs-input");
  });
  it("running when any session runs", () => {
    expect(deriveDockIconState([with_({ kind: "idle" }, "a"), with_({ kind: "running" }, "b")])).toBe("running");
  });
  it("idle when empty or only terminal states", () => {
    expect(deriveDockIconState([])).toBe("idle");
    expect(deriveDockIconState([with_({ kind: "failed", error: "x" })])).toBe("idle");
  });
});

describe("tabNote", () => {
  it("notes idle reaping", () => {
    expect(tabNote(with_({ kind: "closed", reason: "idle_reaped" }))).toBe("closed after idle");
  });
  it("surfaces failure error", () => {
    expect(tabNote(with_({ kind: "failed", error: "exited with status 3" }))).toBe("exited with status 3");
  });
  it("is null for live sessions", () => {
    expect(tabNote(with_({ kind: "running" }))).toBeNull();
  });
});
```

**NOTE:** field/case names above assume specta's camelCase field renaming and the serde
snake_case tags from Task 2 — verify against the regenerated `bindings.gen.ts` and adjust
the test to the REAL generated names before implementing.

- [ ] **Step 2: Run to fail** — `bunx vitest run src/features/dock/model/dock-state.test.ts`.

- [ ] **Step 3: Implement `dock-state.ts`**, run again — PASS.

- [ ] **Step 4: Tokens (stash procedure).** Stash the owner paths (Global Constraints
  command), then in `src/styles/main.css` `@theme` add:

```css
  /* terminal viewport — machine region, deliberately dark in both themes */
  --term-bg: oklch(0.16 0.005 75);
  --term-fg: oklch(0.87 0.01 75);
```

(match the file's existing token comment style; warm-neutral hue family consistent with
the existing dark tokens — read the file's dark `--bg` and keep the same hue angle).
Commit this hunk alone in Step 8's first ship call, then `git stash pop`.

- [ ] **Step 5: IPC + store + hook.** `src/lib/ipc/sessions.ts` mirrors
  `src/lib/ipc/jobs.ts` style (wrap `commands.*`, `unwrap`, export keys).
  `use-sessions.ts`: `useSessions()` = `useQuery({queryKey: sessionsKeys.all, queryFn: listSessions})`.
  Broadcast: add `onSessionsChanged`/`onSessionStateChanged` wrappers in
  `src/lib/ipc/broadcast.ts` (copy the existing listener pattern) and invalidate
  `sessionsKeys.all` in `use-broadcast-invalidation.ts` for both. `useUiStore`: add the
  four actions; `openDock(sessionId?)` sets both fields; every mutation of
  `dockOpen`/`activeSessionId` also fires `setDockFocus(dockOpen, activeSessionId)`
  (side-effect in the components' handlers, NOT inside the store — keep the store pure;
  wire it in `dock-panel.tsx` via `useEffect` on `[dockOpen, activeSessionId]`).
- [ ] **Step 6: Failing component test** `src/features/dock/tab-strip.test.tsx` (RTL,
  role/label queries; fixtures via a local `makeSession` helper mirroring Step 1):
  asserts (a) one tab button per session labeled with its `label`; (b) the active tab has
  `aria-selected="true"`; (c) a `closed after idle` note renders for a reaped session;
  (d) a failed session shows its error text; (e) clicking a tab calls `onSelect(id)`;
  (f) clicking the tab's close control calls `onClose(id)`. Props:
  `TabStrip({ sessions, activeId, onSelect, onClose }: { sessions: SessionInfo[]; activeId: string | null; onSelect: (id: string) => void; onClose: (id: string) => void })`.
  Run to fail, then implement: horizontal chip row (`role="tablist"`), each tab a
  `role="tab"` button — state dot (`--done`/`--failed`/`--warn`/`--ember`/muted per
  state, one dot, no second encoding) + sans label + muted note (from `tabNote`), close
  `×` button `aria-label={"close " + label}`. Fixed 32px height rows, `--r` radius,
  `cursor: default`. Run — PASS.
- [ ] **Step 7: Panel + icon.** `dock-panel.tsx`: renders nothing when `!dockOpen`;
  otherwise an absolutely-positioned right-side slide-over INSIDE the content pane
  (overlay above `[data-scroll-root]`, `w-[600px] max-w-[80vw] h-full`, `--surface`
  background with grain, left `--hairline` border, `--shadow` (floating layer),
  translate-in over `--dur` `--ease`, `motion-reduce:transition-none`): header row
  (serif "Claude" wordmark-weight label + spawn button ("New session", secondary) +
  hide button), `TabStrip`, and the terminal region (Task 10 placeholder `<div>` for
  now). Component test `dock-panel.test.tsx`: closed → nothing in DOM; open with two
  sessions → tablist present; "New session" button calls `spawnSession` (mock IPC via
  the project's `mockIPC` pattern); hide button flips store. `dock-icon.tsx`: renders
  the sidebar button — Phosphor `RobotIcon` (Regular, 20px standalone) wrapped with
  state visuals per `deriveDockIconState` + a transient done-check: keep
  `prevRunning` in a ref; when a previously-running id turns idle/closed, show
  Phosphor `CheckIcon` overlay for 3 s (clear on timer; `motion-reduce` renders it
  statically). `running` → pulsing ring (CSS `@keyframes` opacity ring on a
  `::after`-equivalent span, `--ember`, gated by `motion-reduce:animate-none`);
  `needs-input` → 6px `--warn` badge dot (one encoding, no ring simultaneously).
  In `sidebar.tsx` replace the disabled placeholder (`:66–74`) with `<DockIcon/>`
  wired to `toggleDock()`; active style follows `NavButton` conventions but the dock
  is an overlay, not a surface — the button shows pressed state while `dockOpen`.
  Mount `<DockPanel/>` in `app.tsx` inside the content pane container (sibling of the
  scroll root, absolute overlay; the window never scrolls).
- [ ] **Step 8: Run all frontend tests + gate slice** —
  `bunx vitest run src/features/dock` PASS, `bun run typecheck` (or `just check`'s tsc
  step) clean.
- [ ] **Step 9: Commit via `ship`** — two commits:
  1. paths `src/styles/main.css` — `feat(dock): terminal viewport tokens` (stash-pop
     dance around it; verify `git diff --cached src/styles/main.css` shows ONLY the
     token hunk).
  2. paths `src/stores/ui.ts src/lib/ipc/sessions.ts src/lib/ipc/broadcast.ts src/hooks/use-broadcast-invalidation.ts src/features/dock/ src/components/layout/sidebar.tsx src/app/app.tsx` —
     `feat(dock): slide-over panel, tab strip, sidebar icon states`.

---

### Task 10: Terminal component (xterm lifecycle)

**Files:**
- Create: `src/features/dock/terminal.tsx`
- Create: `src/features/dock/terminal.test.tsx`
- Modify: `src/features/dock/dock-panel.tsx` (replace placeholder)

**Interfaces:**
- Consumes: `attachSession`, `writeSession`, `resizeSession` (Task 9 IPC), tokens
  `--term-bg`/`--term-fg`/`--mono`.
- Produces: `Terminal({ sessionId }: { sessionId: string })` — one xterm instance per
  MOUNTED session; the panel renders only the active tab's terminal (`key={sessionId}`
  so switching tabs unmounts/remounts and replays scrollback — the backend ring makes
  this cheap and correct).

Lifecycle contract (this is the test spec):
1. mount → `new XTerm({fontFamily: cssVar("--mono"), fontSize: 12, theme: {background: cssVar("--term-bg"), foreground: cssVar("--term-fg"), cursor: cssVar("--term-fg"), selectionBackground: cssVar("--ember")}, scrollback: 5000})`;
   `loadAddon(fit)`; `loadAddon(webgl)` inside `try {} catch` with
   `webgl.onContextLoss(() => webgl.dispose())`; `term.open(containerEl)`; `fit.fit()`;
   `attachSession(sessionId, bytes => term.write(bytes))`.
2. `term.onData(d => writeSession(sessionId, d))`.
3. `term.onResize(({cols, rows}) => resizeSession(sessionId, cols, rows))`; a
   `ResizeObserver` on the container calls `fit.fit()` (debounced 100 ms).
4. unmount → disconnect observer, `term.dispose()` (addon disposal rides on it).

- [ ] **Step 1: Failing test with mocked xterm** (`terminal.test.tsx`): `vi.mock("@xterm/xterm", ...)`
  exporting a `Terminal` class capturing constructor options and exposing spies
  (`open`, `write`, `onData`, `onResize`, `loadAddon`, `dispose`); mock both addons;
  mock `@/lib/ipc/sessions` (`attachSession` resolves and stores the callback,
  `writeSession`/`resizeSession` spies). Assert: (a) mount calls `open` then
  `attachSession` with the session id; (b) invoking the stored attach callback with
  `new Uint8Array([104, 105])` reaches `term.write`; (c) the registered `onData`
  handler forwards `"ls\n"` to `writeSession("s1", "ls\n")`; (d) unmount calls
  `dispose`; (e) a scripted byte stream of three chunks arrives in order (PRD test
  requirement). jsdom lacks `ResizeObserver` — the existing `src/test/setup.ts`
  polyfill covers it.

- [ ] **Step 2: Run to fail** — `bunx vitest run src/features/dock/terminal.test.tsx`.

- [ ] **Step 3: Implement** `terminal.tsx` per the contract; read CSS vars once at mount
  via `getComputedStyle(document.documentElement).getPropertyValue(...)` with fallbacks;
  container `<div className="h-full min-h-0" style={{ backgroundImage: "none" }}>`
  (grain opt-out) — the terminal region is the ONLY mono surface in the dock.

- [ ] **Step 4: Run** — PASS. Swap into `dock-panel.tsx`
  (`{active && <Terminal key={active.id} sessionId={active.id} />}`), re-run panel
  tests.

- [ ] **Step 5: Commit via `ship`** — paths:
  `src/features/dock/terminal.tsx src/features/dock/terminal.test.tsx src/features/dock/dock-panel.tsx package.json bun.lock` —
  `feat(dock): xterm terminal with fit/webgl, attach/write/resize wiring`.

### Task 11: Scheduler due-math (pure)

**Files:**
- Create: `src-tauri/src/scheduler.rs` (module root: `pub mod due;`)
- Create: `src-tauri/src/scheduler/due.rs`
- Modify: `src-tauri/src/lib.rs` (`mod scheduler;`)

**Interfaces:**
- Produces:
  - `pub struct ScheduleSpec { pub hour: u32, pub minute: u32, pub catchup: chrono::Duration }` (`Debug, PartialEq, Clone`)
  - `pub fn parse_spec(spec: &str) -> Option<ScheduleSpec>` — grammar exactly
    `daily@HH:MM;catchup=<N>h` (both parts required; anything else → `None`).
  - `pub fn is_due(spec: &ScheduleSpec, last_success: Option<chrono::NaiveDateTime>, now: chrono::NaiveDateTime) -> bool`
    — due iff `now >= today's slot` AND (`last_success` is `None` OR
    `now - last_success >= spec.catchup`). All times LOCAL naive; the runtime converts.
  - `pub fn retry_backoff(consecutive_failures: u32) -> chrono::Duration` —
    `min(2^(n-1) minutes, 60 minutes)` for n ≥ 1 (1, 2, 4, … capped at 60).
- Consumes: `chrono` (already a dep with `std`; verify `NaiveDateTime` needs no extra
  feature — it doesn't).

- [ ] **Step 1: Failing table tests:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, NaiveDate, NaiveDateTime};

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, mo, d).unwrap().and_hms_opt(h, mi, 0).unwrap()
    }

    fn nightly() -> ScheduleSpec {
        parse_spec("daily@00:00;catchup=20h").unwrap()
    }

    #[test]
    fn parse_round_trips_fields() {
        let s = parse_spec("daily@02:30;catchup=20h").unwrap();
        assert_eq!((s.hour, s.minute), (2, 30));
        assert_eq!(s.catchup, Duration::hours(20));
    }

    #[test]
    fn parse_rejects_malformed() {
        for bad in ["daily@25:00;catchup=20h", "daily@00:00", "weekly@00:00;catchup=20h", "", "daily@0:0;catchup=h"] {
            assert!(parse_spec(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn never_run_and_slot_passed_is_due() {
        assert!(is_due(&nightly(), None, at(2026, 7, 22, 0, 1)));
    }

    #[test]
    fn before_todays_slot_not_due() {
        let s = parse_spec("daily@23:00;catchup=20h").unwrap();
        assert!(!is_due(&s, None, at(2026, 7, 22, 8, 0)));
    }

    #[test]
    fn slept_through_slot_runs_once_on_wake() {
        // succeeded yesterday 00:05, Mac slept through 00:00, wakes 08:00 → 32h ago → due
        assert!(is_due(&nightly(), Some(at(2026, 7, 21, 0, 5)), at(2026, 7, 22, 8, 0)));
    }

    #[test]
    fn already_ran_today_not_due() {
        // ran 00:05 today, now 09:00 → 9h < 20h catch-up → quiet
        assert!(!is_due(&nightly(), Some(at(2026, 7, 22, 0, 5)), at(2026, 7, 22, 9, 0)));
    }

    #[test]
    fn multiple_missed_days_still_one_run() {
        // is_due is level-triggered; the runtime writes last_success on completion,
        // so a 3-day gap yields exactly one run — due before, not-due after success.
        let before = is_due(&nightly(), Some(at(2026, 7, 18, 0, 5)), at(2026, 7, 22, 10, 0));
        let after = is_due(&nightly(), Some(at(2026, 7, 22, 10, 5)), at(2026, 7, 22, 10, 6));
        assert!(before);
        assert!(!after);
    }

    #[test]
    fn backoff_doubles_and_caps() {
        assert_eq!(retry_backoff(1), Duration::minutes(1));
        assert_eq!(retry_backoff(3), Duration::minutes(4));
        assert_eq!(retry_backoff(10), Duration::minutes(60));
    }
}
```

- [ ] **Step 2: Run to fail** — `cargo test -p katto scheduler::due`.

- [ ] **Step 3: Implement** (string split on `';'` / `'@'` / `'='`, `u32` parses with
  range checks h<24 m<60; `is_due` builds today's slot with
  `now.date().and_hms_opt(spec.hour, spec.minute, 0)`; backoff via
  `Duration::minutes(1 << (n-1).min(6))` then cap).

- [ ] **Step 4: Run** — PASS.

- [ ] **Step 5: Commit via `ship`** — paths:
  `src-tauri/src/scheduler.rs src-tauri/src/scheduler/due.rs src-tauri/src/lib.rs` —
  `feat(scheduler): anacron due-math and spec parser`.

---

### Task 12: Scheduler runtime — tick, wake observer, commands

**Files:**
- Create: `src-tauri/src/scheduler/runtime.rs` (`pub mod runtime;` in scheduler.rs)
- Create: `src-tauri/src/commands/scheduler.rs`
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs` (collect + setup spawn)
- Modify: `src-tauri/src/broadcast.rs` (reuse `EventsAppended`; no new event needed)

**Interfaces:**
- Consumes: Task 11 (`parse_spec`, `is_due`, `retry_backoff`), Task 1
  (`db::scheduled_jobs::*`), `DbHandle::call`, `db::events::record`,
  `chrono::Local::now().naive_local()`, objc2-app-kit `NSWorkspaceDidWakeNotification`
  (verified contract section).
- Produces:
  - `pub struct SchedulerHandle { nudge: tokio::sync::mpsc::UnboundedSender<()> }` in
    managed state (`AppState.scheduler: SchedulerHandle`).
  - `pub fn start(app: AppHandle) -> SchedulerHandle` — spawns the evaluate loop:
    `tokio::select!` over `tokio::time::interval(60s)` tick and `nudge.recv()`; each
    evaluation: `db::scheduled_jobs::list` → for enabled rows, `parse_spec` (a `None`
    parse records events row `scheduler_bad_spec {name, spec}` ONCE per launch — keep a
    `HashSet<String>` of already-reported names) → skip rows whose in-memory
    `next_retry_at` is in the future → `is_due(spec, last_success_local, now_local)` →
    run via the registry. `last_success_at` is stored as `"YYYY-MM-DD HH:MM:SS"` LOCAL
    (matches `is_due`'s naive-local math; document this on the column accessor).
  - Registry: `async fn run_job(app: &AppHandle, name: &str) -> std::result::Result<(), String>` —
    `match name { "nightly-curation" => crate::curation::run(app).await, _ => Err(...) }`
    (Task 14 provides `curation::run`; until then leave a
    `Err("unknown scheduled job".into())` arm and the curation arm commented with a
    `// Task 14` marker — the scheduler compiles and tests independently).
    Success → `set_last_success(now_local_string)` + reset failure counter; failure →
    counter += 1, `next_retry_at = now + retry_backoff(counter)`, events row
    `scheduler_job_failed {name, error, retry_in_secs}`. A run in flight guards
    re-entry (an `AtomicBool` per name — a 60 s tick during a long curation must not
    double-run; PRD: never runs twice for one slot).
  - Wake observer (macOS, in `start`): register
    `addObserverForName_object_queue_usingBlock` on
    `NSWorkspace::sharedWorkspace().notificationCenter()` for
    `NSWorkspaceDidWakeNotification`; the block sends `()` on `nudge`. Keep the observer
    token `Retained` in a `static OnceLock` (or leak deliberately with a comment — it
    lives for the process). Non-macOS: skip (app is macOS-only anyway).
  - Commands (via **`add-tauri-command`**):
    `get_scheduler_state() -> Result<Vec<ScheduledJob>>` (repo list verbatim),
    `run_scheduled_job_now(name: String) -> Result<()>` (sends a targeted run —
    implement as: mark the row due by clearing the failure gate and invoking `run_job`
    directly in a spawned task; events row `scheduler_manual_run {name}`),
    `set_scheduled_job(name: String, hour: u32, minute: u32, enabled: bool) -> Result<()>`
    (validates via `parse_spec(&format!("daily@{hour:02}:{minute:02};catchup=20h"))`,
    writes with `db::scheduled_jobs::update` — catch-up stays 20 h, not user-editable).

- [ ] **Step 1: Failing tests** — the evaluate DECISION is already pure (Task 11); test
  the runtime's glue that is testable without tauri: extract
  `pub(crate) fn evaluate_row(job: &db::scheduled_jobs::ScheduledJob, now_local: NaiveDateTime, next_retry_at: Option<NaiveDateTime>) -> RowVerdict`
  (`enum RowVerdict { Run, NotDue, Disabled, BadSpec, Backoff }`) with tests:

```rust
    #[test]
    fn disabled_row_never_runs() { /* enabled=false → Disabled even when due */ }
    #[test]
    fn bad_spec_is_flagged_not_fatal() { /* spec "garbage" → BadSpec */ }
    #[test]
    fn backoff_window_suppresses_due_row() { /* due but next_retry_at in future → Backoff */ }
    #[test]
    fn due_enabled_row_runs() { /* seed spec, old last_success → Run */ }
```

(Write them concretely with the Task 11 helpers — construct `ScheduledJob` literals.)

- [ ] **Step 2: Run to fail**, **Step 3: implement** `evaluate_row` + the loop + observer
  + commands per the interface block, **Step 4:**
  `cargo test -p katto scheduler` PASS; `cargo clippy -p katto` clean (the objc2 block
  code will need `#[cfg(target_os = "macos")]` and careful `unsafe` blocks — one
  `// SAFETY:` comment each, matching `capture.rs` style).

- [ ] **Step 5: Wire.** `AppState` gains `scheduler: SchedulerHandle`; `lib.rs` setup
  spawns `scheduler::runtime::start` after the pool; register commands + regenerate
  bindings (`cargo test -p katto export_bindings`).

- [ ] **Step 6: Commit via `ship`** — paths:
  `src-tauri/src/scheduler/runtime.rs src-tauri/src/scheduler.rs src-tauri/src/commands/scheduler.rs src-tauri/src/commands.rs src-tauri/src/state.rs src-tauri/src/lib.rs src-tauri/Cargo.toml Cargo.lock src/lib/ipc/bindings.gen.ts` —
  `feat(scheduler): 60s tick with catch-up, wake nudge, manual-run commands`.

---

### Task 13: Notification click → deep-link delegate

Closes the known gap in `notify.rs` (`macos::deliver` ignores `_url`): Phase 6's user
story "clicking the notification lands me on the Ideas page" requires it.

**Files:**
- Modify: `src-tauri/src/notify.rs`

**Interfaces:**
- Consumes: `objc2-user-notifications` (already a dep; the delegate contract is in the
  verified-contracts section — **re-verify exact generated names on docs.rs before
  coding**), `parse_deep_link`, `broadcast::deep_link_opened`, `window::show_main`.
- Produces: notifications delivered with `userInfo["katto_url"] = <url>`; a
  `UNUserNotificationCenterDelegate` (via `objc2::define_class!`) whose
  `didReceiveNotificationResponse` reads `katto_url`, calls
  `parse_deep_link`, then on the main thread `window::show_main(app)` +
  `broadcast::deep_link_opened(app, &route.as_wire())`; delegate installed once in
  `notify` setup (add `pub fn init(app: &AppHandle)` called from the lib.rs setup hook
  after `tray::create`). The frontend's existing `useDeepLinkRouter` handles the rest
  (Task 16 adds the `"dock"` case).

- [ ] **Step 1:** No pure logic to TDD here beyond `parse_deep_link` (already tested +
  Task 8's dock case) — this is a thin objc2 glue task. Write the delegate:
  `define_class!` with superclass `NSObject`, protocol
  `UNUserNotificationCenterDelegate`, an ivar holding `AppHandle`;
  implement the response method; set
  `UNUserNotificationCenter::currentNotificationCenter().setDelegate(Some(...))`;
  store the delegate `Retained` in a `static OnceLock` so it outlives the call. In
  `deliver`, set `content.setUserInfo(...)` with the url string (build the
  `NSDictionary<NSString, NSObject>` via objc2-foundation helpers). Mind: delegate
  callbacks arrive off the main thread → hop via `app.run_on_main_thread`.
  **completionHandler must be called** (`block` arg) — invoke it unconditionally.

- [ ] **Step 2: Build + clippy** — `cargo clippy -p katto` clean;
  `cargo test -p katto notify` still green (route tests).

- [ ] **Step 3: Manual checkpoint note** — this is hardware/UI-verifiable only; it is
  covered by the Task 20 owner checklist (curation notification click). State that in
  the commit body.

- [ ] **Step 4: Commit via `ship`** — paths: `src-tauri/src/notify.rs src-tauri/src/lib.rs` —
  `feat(notify): notification click routes through katto:// deep links`.

---

### Task 14: Nightly curation runner

**Files:**
- Create: `src-tauri/src/curation.rs`
- Modify: `src-tauri/src/scheduler/runtime.rs` (uncomment the registry arm)
- Modify: `src-tauri/src/lib.rs` (`mod curation;`)
- Modify: `src-tauri/src/commands/settings.rs` (new keys, see below)

**Interfaces:**
- Consumes: `SessionPool::spawn` (full `SessionTask` with `permission_allow`),
  session state via a completion watch (below), Task 1 repos
  (`raw_signal::{judged_counts_since, unjudged_count, prune_judged_older_than_days}`,
  `ideas::count_since`, `scheduled_jobs`), `JobRuntime::spawn`, `notify::notify`,
  settings keys `discovery_enabled` (`"true"`/`"false"`, default `"false"`),
  `hyperframes_path` (absolute path to the hyper-frames checkout, default unset),
  `db::settings::get`.
- Produces: `pub async fn run(app: &AppHandle) -> std::result::Result<(), String>` —
  the scheduler-registry entry; also invoked by the palette's "run nightly curation
  now" via `run_scheduled_job_now`.

Flow (all inside one `jobs.spawn("nightly_curation", "Nightly curation", None, work)` —
`run` wraps the spawn and awaits the job's oneshot? No — simpler and consistent:
`run` IS the work body; the scheduler registry calls
`state.jobs.spawn("nightly_curation", "Nightly curation", None, |ctx| curation::work(app, ctx))`
and `run_job` treats the JOB's terminal state as the scheduler's success signal: have
`work` return the final `Result<(), String>` and `run_job` await a oneshot resolved by
`work` itself just before returning — implement it as
`curation::run(app) -> Result<(), String>` that internally spawns the job AND awaits a
`tokio::sync::oneshot` the work body resolves; document why: the scheduler needs the
outcome to gate `last_success_at`, and the jobs row is the visibility contract):

1. `let run_started = chrono local "%Y-%m-%d %H:%M:%S"` (matches SQLite `datetime('now','localtime')`
   format — the session inserts with `datetime('now')` UTC… **so use UTC here**:
   `chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")` — the curation session's
   `judged_at`/`first_seen` are written by sqlite `datetime('now')` = UTC. Local-vs-UTC
   matters: `judged_counts_since`/`count_since` compare against sqlite-UTC strings.)
2. Claude present? settings `claude_path` else `detect_claude()`; missing → events row
   `curation_skipped {reason: "claude not on PATH"}` + `Err` (scheduler backs off; no
   crash loop — PRD).
3. Discovery (optional): if `discovery_enabled == "true"` and `hyperframes_path` set:
   run `uv run studio-discover --db <app_data_dir>/katto.db` with
   `current_dir = <hyperframes_path>/tools/studio/discovery` via
   `tokio::process::Command` (env from parent; 15-min timeout via
   `tokio::time::timeout`), progress-noting via `ctx.progress(0.2, Some("discovery"))`.
   Failure/timeout → `discovery_failed = true`, events row
   `curation_discovery_failed {stderr_tail}` (last ~500 chars), continue.
4. `unjudged_count == 0` and no discovery ran? Still proceed (the session also
   novelty-guards; a no-op run is cheap and the count may race discovery) — but if
   claude present and unjudged is 0 AND discovery didn't run, short-circuit: events row
   `curation_done {kept: 0, discarded: 0, noop: true}`, notification skipped, `Ok(())`.
5. Spawn the dock session: `SessionTask { label: "ideas: nightly", cwd: app_data_dir, initial_prompt: Some(curation_prompt(&db_path)), append_system_prompt: None, permission_mode: None, permission_allow: vec!["Bash(sqlite3:*)".into()] }`.
6. Await completion: subscribe to the session's terminal/idle transition — add
   `pub fn watch_first_stop(&self, id: &str) -> tokio::sync::oneshot::Receiver<std::result::Result<(), String>>`
   to `SessionPool` (resolves `Ok` on first `Running→Idle` via HookStop, `Err(error)`
   on Failed/Closed-before-stop; 30-min `tokio::time::timeout` wrapper here — on
   timeout, events row `curation_timeout`, leave the session alive (visible, D18), and
   return `Err`).
7. On `Ok`: `let (kept, discarded) = judged_counts_since(&conn, &run_started)`;
   `let new_ideas = ideas::count_since(&conn, &run_started)`; prune:
   `prune_judged_older_than_days(&conn, 90)`; events row
   `curation_done {kept, discarded, new_ideas, discovery_failed}`;
   `broadcast::ideas_changed(app)`; notification:
   title `"Nightly curation"`, body `"kept {kept} / discarded {discarded}"` (+
   `" — discovery failed"` suffix when it did), url `"katto://ideas"`.
8. Return `Ok(())` → scheduler writes `last_success_at`. Any `Err` path already left an
   events row (the jobs framework adds `job_failed`).

`pub(crate) fn curation_prompt(db_path: &Path) -> String` — a `format!` over the const
template below (the ONLY judgment text; criteria verbatim from the studio-ideas skill,
schema adapted to katto — see verified-contracts):

```text
Curate the idea backlog in the katto SQLite database at {db}.

Work ONLY through the sqlite3 CLI against that path. Do not read any other files.

1. Read only the unjudged delta. First:
   sqlite3 {db} "SELECT source, count(*) FROM raw_signal WHERE judged_at IS NULL GROUP BY source;"
   Then pull compact batches (JSON mode), aggregator/video rows and
   youtube-comments:* rows SEPARATELY, capped (~120 aggregator rows, ~40 comment rows
   per batch); judge a batch, mark it, then pull the next. Never the whole table.
2. Novelty guard: existing idea titles —
   sqlite3 {db} "SELECT title FROM ideas WHERE status IN ('backlog','promoted');"
   and existing project folders —
   sqlite3 {db} "SELECT slug, title FROM projects;"
3. Judge each row keep or discard — binary, qualitative, with a one-line why-pursue
   rationale. NEVER a number, score, rank, percentage, or grade. Lenses:
   - Fit: a real computing / system-design / dev-tools angle that is on-brand.
     Off-topic, listicle-bait, pure news → discard.
   - Novelty: not already an idea or a made project; a near-duplicate of covered
     ground → discard (or keep as a deliberately fresh angle, said so in the
     rationale).
   - Demand shape (comment rows): cluster into themes; a RECURRING ask is a strong
     keep — capture 1-2 representative quotes and the count. One-off praise is not
     demand.
   When unsure, discard — the backlog is small on purpose.
4. For keepers, suggest a format kind: short (single tight mechanism, broad-appeal
   quick hit) / long (full system-design build-up) / series (too big for one video).
   It is a suggestion the human confirms — never a grade.
5. Insert keepers into ideas:
   id = lower(hex(randomblob(8))), status='backlog',
   type mapped from source (youtube-comments:* → comment_demand, youtube:* → mirror,
   hn / reddit:* / lobsters / dailydev → trend),
   kind = your suggestion, kind_source='ai', kind_why = one line,
   rationale = the one-line why-pursue,
   source, source_url, source_title from the raw row, raw_signal_id = its id,
   first_seen = datetime('now'),
   evidence_json = json('{{"lean":"hold|lean|strong", ...source metrics, quotes for
   comment_demand}}') — lean is a categorical meter hint, never a number.
   The title is YOUR framing of the video, not necessarily the raw item's title.
6. Mark EVERY row you read this pass:
   judged_at = datetime('now'), judged_verdict = 'kept' or 'discarded'.
   Discarded rows stay as the audit trail — never delete, never re-judge.
7. Finish with a one-paragraph summary: kept N by type, discarded M, titles with
   suggested kinds. Do not promote anything, do not act on any idea — the
   make-or-not call is the human's.
```

- [ ] **Step 1: Failing tests** — (a) `curation_prompt` assertions (contains the db
  path, contains `judged_at IS NULL`, contains `NEVER a number`, contains
  `status='backlog'`, does NOT contain the word `score` outside the negation — simple
  `assert!(prompt.contains(...))` set); (b) the count/prune repo fns are already tested
  (Task 1); (c) `watch_first_stop` behavior — extend the Task 6 pool tests:

```rust
    #[tokio::test]
    async fn watch_first_stop_resolves_on_hook_stop() {
        let pool = SessionPool::new();
        let id = pool.spawn_headless(task("t"), Program::Custom("bash -c 'cat'".into())).await.unwrap();
        let rx = pool.watch_first_stop(&id);
        pool.apply_hook_event(HookEvent::Stop { session_id: id.clone() }); // test seam, pub(crate)
        assert!(rx.await.unwrap().is_ok());
        pool.close(&id, CloseReason::UserClosed).await.unwrap();
    }

    #[tokio::test]
    async fn watch_first_stop_errors_on_failure() {
        let pool = SessionPool::new();
        let id = pool.spawn_headless(task("t"), Program::Custom("bash -c 'exit 7'".into())).await.unwrap();
        let rx = pool.watch_first_stop(&id);
        assert!(rx.await.unwrap().is_err());
    }
```

- [ ] **Step 2: Run to fail** — `cargo test -p katto curation watch_first_stop`.

- [ ] **Step 3: Implement** per the flow; settings keys `discovery_enabled` /
  `hyperframes_path` join `read_settings`/`SettingsPatch` in `commands/settings.rs`
  (string k/v; patch-writable). Uncomment the scheduler registry arm.

- [ ] **Step 4: Run** — `cargo test -p katto` PASS.

- [ ] **Step 5: Commit via `ship`** — paths:
  `src-tauri/src/curation.rs src-tauri/src/lib.rs src-tauri/src/scheduler/runtime.rs src-tauri/src/sessions/pool.rs src-tauri/src/commands/settings.rs src/lib/ipc/bindings.gen.ts` —
  `feat(curation): nightly dock-session curation with DB-delta summary and prune`.

---

### Task 15: Cut-plan re-route through the dock

**Files:**
- Modify: `src-tauri/src/jobs/pipeline.rs` (planner stage)
- Modify: `src-tauri/src/commands/pipeline.rs` (`PlannerKind::Dock`, `resolve_planner`)
- Create: `src-tauri/src/sessions/planfile.rs` (pure verdict logic) + `pub mod planfile;`
- Modify: `src-tauri/src/commands/settings.rs` (`dock_planning` key, default `"true"`)

**Interfaces:**
- Consumes: `PlannerKind` (`src-tauri/src/jobs/pipeline.rs:43`), `resolve_planner`
  (`src-tauri/src/commands/pipeline.rs:274`),
  `katto_engine::planner::{parse_cuts_json, correction_message, PlanError, CUT_DECIDER_PROMPT}`,
  `katto_engine::bundle::CUTS_JSON`, `SessionPool::{spawn, write, watch_first_stop}`,
  settings `dock_planning`.
- Produces:
  - `PlannerKind::Dock { claude_path: PathBuf }` — chosen by `resolve_planner` when
    `dock_planning != "false"` AND claude resolves; `Subprocess` when dock disabled;
    `Http` fallback unchanged.
  - `planfile.rs`: `pub enum PlanFileVerdict { Missing, Valid(katto_engine::schema::Cuts), Invalid { errors_message: String } }`
    and `pub fn evaluate_plan_file(read: std::io::Result<String>) -> PlanFileVerdict`
    (NotFound → Missing; other io error → Invalid with the io message; content →
    `parse_cuts_json`, `PlanError::Invalid{error, ..}` → Invalid with
    `correction_message`-ready text, Ok → Valid).
  - Pipeline dock stage `async fn plan_via_dock(app, pool, bundle_root, claude_path) -> Result<Cuts, String>`:
    1. Spawn session: label `"cut plan: <bundle file stem>"`, cwd = bundle_root,
       `append_system_prompt: Some(CUT_DECIDER_PROMPT.into())`,
       `permission_mode: Some("acceptEdits".into())`, initial prompt (exact text):
       `"Read transcript.json in this directory and produce the rough-cut plan. Write the result as cuts.json in this directory, exactly matching the schema from your instructions — a single JSON object, no prose in the file. Then stop."`
    2. Poll loop (2 s interval, 10-min `tokio::time::timeout` overall): read
       `bundle_root.join(CUTS_JSON)` → `evaluate_plan_file`. `Valid` → done.
       `Invalid` → first time: rename the bad file to `cuts.invalid-1.json` (audit),
       `pool.write(id, correction_text.as_bytes())` then `pool.write(id, b"\r")`
       (push the correction INTO the visible session — retry contract, one retry);
       second `Invalid` → fail the stage with the validation message. `Missing` →
       keep polling; if `watch_first_stop` already resolved `Err` (session died) →
       fail with its error.
    3. On success leave the session open (it idles → reaper closes it later; its tab
       shows the run); on failure leave it open too (D18).
    The stage then continues EXACTLY like the subprocess path: the pipeline still owns
    `write_json_atomic(bundle_root.join(CUTS_JSON), &cuts)`? — NO: the session already
    wrote the file and `evaluate_plan_file` validated it; re-serialize through
    `write_json_atomic` anyway so the on-disk artifact is normalized and
    atomically-written (cheap, keeps the invariant "artifact writes are atomic", and
    equals the subprocess path's post-condition). Then `rough_cut_planned` events row
    etc. — reuse the existing code path after the planner match.

- [ ] **Step 1: Failing tests** for `evaluate_plan_file` (in `planfile.rs`; craft a
  minimal valid cuts JSON by reading an existing engine fixture —
  `crates/katto-engine/tests/fixtures/` has cuts fixtures; embed the smallest valid one
  as a `const` in the test module):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error, ErrorKind};

    #[test]
    fn missing_file_is_missing() {
        let verdict = evaluate_plan_file(Err(Error::new(ErrorKind::NotFound, "no")));
        assert!(matches!(verdict, PlanFileVerdict::Missing));
    }

    #[test]
    fn unparseable_json_is_invalid_with_message() {
        match evaluate_plan_file(Ok("not json".into())) {
            PlanFileVerdict::Invalid { errors_message } => assert!(!errors_message.is_empty()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn valid_cuts_json_parses() {
        let fixture = include_str!("../../../crates/katto-engine/tests/fixtures/<PICK-REAL-FIXTURE>.json");
        assert!(matches!(evaluate_plan_file(Ok(fixture.into())), PlanFileVerdict::Valid(_)));
    }
}
```

(Resolve `<PICK-REAL-FIXTURE>` by listing that fixtures dir — pick a known-valid cuts
file used by engine tests; adjust the relative `include_str!` path from
`src-tauri/src/sessions/`.)

- [ ] **Step 2: Run to fail** — `cargo test -p katto planfile`.

- [ ] **Step 3: Implement** `planfile.rs`, then thread `PlannerKind::Dock` through
  `resolve_planner` (order: `dock_planning` enabled + claude → Dock; claude only →
  Subprocess is now the dock-disabled path; keychain → Http; none → `Error::NoPlanner`)
  and the pipeline stage. The pipeline job already runs inside the jobs framework — the
  dock session ADDITIONALLY appears as its own `claude_session` job (uniform rule,
  Global Constraints). `settings.rs`: add `dock_planning` to reads + patch.

- [ ] **Step 4: Run** — `cargo test -p katto` PASS. (No live claude test here — the
  Task 20 owner checklist covers the real run.)

- [ ] **Step 5: Commit via `ship`** — paths:
  `src-tauri/src/sessions/planfile.rs src-tauri/src/sessions.rs src-tauri/src/jobs/pipeline.rs src-tauri/src/commands/pipeline.rs src-tauri/src/commands/settings.rs src/lib/ipc/bindings.gen.ts` —
  `feat(pipeline): route cut planning through a visible dock session`.

### Task 16: Settings dock section, palette commands, dock deep-link routing

**Files:**
- Create: `src/lib/ipc/scheduler.ts`
- Create: `src/features/settings/components/dock-section.tsx` (+ test)
- Modify: `src/features/settings/settings-page.tsx`
- Modify: `src/app/commands.ts`
- Modify: `src/hooks/use-deep-link-router.ts` (+ its test if one exists)

**Interfaces:**
- Consumes: bindings `getSchedulerState`, `runScheduledJobNow`, `setScheduledJob`,
  `ScheduledJob` type (Task 12), settings hook `useSettings()` + patch mutation
  (existing — read `src/features/settings/` to copy the section pattern), settings keys
  `idle_reap_minutes`, `dock_planning`, `discovery_enabled`, `hyperframes_path`,
  `useUiStore.openDock`, `spawnSession`.
- Produces:
  - `src/lib/ipc/scheduler.ts`: `schedulerKeys = { all: ["scheduler"] as const }`,
    `getSchedulerState(): Promise<ScheduledJob[]>`, `runScheduledJobNow(name)`,
    `setScheduledJob(name, hour, minute, enabled)`.
  - `DockSection` on the Settings page (next to `ClaudeSection`):
    - Idle-reap select: options 2 / 5 / 10 minutes (writes `idle_reap_minutes`).
    - Nightly curation: enabled switch + time input (HH:MM) reading
      `getSchedulerState()` row `nightly-curation` (parse hour/minute from the spec
      string with a tiny local parser — test it), writing via `setScheduledJob`;
      "Run now" secondary button → `runScheduledJowNow("nightly-curation")` — spelled
      `runScheduledJobNow`; last-success line in muted text ("last ran 22 Jul, 00:04"
      — or "never").
    - Discovery: switch (`discovery_enabled`) + path input (`hyperframes_path`),
      helper line: "needs uv and a hyper-frames checkout".
    - Cut planning via dock: switch (`dock_planning`).
    Copy in plain sentence-case; controls say what they do; no eyebrows.
  - Palette commands registered in `registerAppCommands` (group "AI", matching existing
    entries): `new claude session` (spawn `{label: "session", cwd: <studio_root from settings query>, ...}`
    then `openDock(id)`), `open dock` (`openDock()`), `run nightly curation now`
    (`runScheduledJobNow` + toast on success — follow how existing palette commands
    surface results).
  - Deep-link: `resolveDeepLink` gains `"dock"` → new kind; `useDeepLinkRouter` handles
    it with `useUiStore.getState().openDock()` (match the file's existing dispatch
    style).

- [ ] **Step 1: Failing tests** — `dock-section.test.tsx`: (a) renders the three
  controls by accessible label ("Idle sessions close after", "Nightly curation",
  "Run now", "Cut planning in the dock", "Discovery"); (b) spec `daily@02:30;catchup=20h`
  renders 02:30 in the time input; (c) "Run now" click calls the mocked
  `runScheduledJobNow` with `nightly-curation`; (d) toggling curation off calls
  `setScheduledJob("nightly-curation", 2, 30, false)`. Mock IPC per project pattern
  (`mockIPC` + `clearMocks` in `src/test/setup.ts` is global). Plus a
  `resolveDeepLink("dock")` unit case in the router's existing test file (create
  `use-deep-link-router.test.ts` case following the file's current tests — read it
  first; if only `resolveDeepLink` is exported-pure, test that).

- [ ] **Step 2: Run to fail** — `bunx vitest run src/features/settings src/hooks`.

- [ ] **Step 3: Implement** all files; use existing `Switch`/`Select`/`Input` primitives
  from `src/components/ui/` (check what exists — Phase 1–5 built select/radio-group/
  slider; add via shadcn CLI only if a primitive is genuinely missing).

- [ ] **Step 4: Run** — PASS; `bunx vitest run src` full frontend suite green.

- [ ] **Step 5: Commit via `ship`** — paths:
  `src/lib/ipc/scheduler.ts src/features/settings/ src/app/commands.ts src/hooks/use-deep-link-router.ts` (+ test files) —
  `feat(settings): dock section (reap, curation schedule, discovery, dock planning) + palette commands`.

---

### Task 17: VFX cockpit backend — scaffold, classify, watch, commands

**Files:**
- Create: `src-tauri/src/vfx.rs`
- Create: `src-tauri/src/commands/vfx.rs`
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs` (mod + collect + setup),
  `src-tauri/src/broadcast.rs` (`VfxRenderLanded`)

**Interfaces:**
- Consumes: `db::projects` repo (read a project's `root_path` by slug — check the exact
  fn, likely `get`/`by_slug` in `src-tauri/src/db/projects.rs`), `notify` crate watcher
  pattern from `volumes.rs:96` (thread + `recommended_watcher`), `SessionPool::spawn`,
  `db::events::record`, `broadcast` pattern.
- Produces:
  - `vfx.rs` pure fns (TDD):
    - `pub fn effect_slug(name: &str) -> Option<String>` — kebab-case a display name
      (lowercase, spaces/underscores → `-`, strip non `[a-z0-9-]`, collapse `--`,
      trim `-`; empty → None).
    - `pub struct RenderLanding { pub effect: String, pub file_name: String }`
    - `pub fn classify_render_event(vfx_root: &Path, changed: &Path) -> Option<RenderLanding>`
      — Some iff `changed` is under `vfx_root`, exactly one directory deep
      (`vfx/<effect>/<file>`), extension `mp4`/`mov` (case-insensitive), and the file
      name doesn't start with `.` (dotfiles/`.tmp` excluded).
  - `pub struct VfxEffect { pub effect: String, pub path: String, pub renders: Vec<String> }` (`Serialize, specta::Type`)
  - `pub fn list_effects(project_root: &Path) -> Vec<VfxEffect>` — scans
    `assets/vfx/*/` (folders are truth), renders sorted newest-first by mtime.
  - Watcher: `pub fn start_watch(app: AppHandle)` — one background thread watching each
    known project's `assets/vfx/` (`RecursiveMode::Recursive`; collect project roots
    via the projects repo at start AND re-collect on `ProjectsChanged`-triggering
    mutations — simplest correct approach: re-scan the watch list every 60 s in the
    same thread via `watcher.watch`/`unwatch` diffs; the create-effect command also
    ensures its project is watched). On `EventKind::Create`/`Modify` for a classified
    path: debounce by size-stability (volumes.rs lesson — poll `metadata().len()`
    twice 500 ms apart until equal, cap 30 s) then: events row `vfx_render_landed`
    `{project: slug, effect, file}` + `broadcast::vfx_render_landed(app, slug, effect, file)`.
    Dedupe repeat Modify storms per file with a `HashMap<PathBuf, Instant>` cooldown
    (ignore re-fires within 10 s of a landing). Watcher-thread death → events row
    `vfx_watcher_died` (mirror volumes.rs).
  - Commands (**`add-tauri-command`**):
    `create_vfx_effect(project_slug: String, name: String) -> Result<String>` —
    resolve project root, `effect_slug(&name)` (None → `Error::InvalidName` — reuse or
    add a typed variant), `std::fs::create_dir_all(root/assets/vfx/<slug>)`, events row
    `vfx_effect_created`, spawn dock session
    `SessionTask { label: format!("vfx: {slug}"), cwd: <the effect dir>, initial_prompt: Some(format!("This is the VFX workspace for the effect \"{name}\" of the video project \"{title}\". Build it here with your usual HyperFrames/Remotion toolchain; render output lands in this folder.")), append_system_prompt: None, permission_mode: None, permission_allow: vec![] }`,
    open dock: returns the session id — the frontend opens the dock with it. Return
    `session_id`.
    `list_vfx_effects(project_slug: String) -> Result<Vec<VfxEffect>>`.
  - Broadcast: `VfxRenderLanded { pub slug: String, pub effect: String, pub file: String }`
    + helper `pub fn vfx_render_landed(app, slug, effect, file)`.

- [ ] **Step 1: Failing pure tests** in `vfx.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn effect_slug_kebabs_and_strips() {
        assert_eq!(effect_slug("Intro Glitch"), Some("intro-glitch".into()));
        assert_eq!(effect_slug("  éclair_flash 2 "), Some("clair-flash-2".into()));
        assert_eq!(effect_slug("!!!"), None);
    }

    #[test]
    fn classify_accepts_one_level_mp4_and_mov() {
        let root = Path::new("/p/assets/vfx");
        let hit = classify_render_event(root, Path::new("/p/assets/vfx/intro-glitch/final.MP4")).unwrap();
        assert_eq!(hit.effect, "intro-glitch");
        assert_eq!(hit.file_name, "final.MP4");
        assert!(classify_render_event(root, Path::new("/p/assets/vfx/x/out.mov")).is_some());
    }

    #[test]
    fn classify_rejects_wrong_depth_ext_and_dotfiles() {
        let root = Path::new("/p/assets/vfx");
        assert!(classify_render_event(root, Path::new("/p/assets/vfx/loose.mp4")).is_none());
        assert!(classify_render_event(root, Path::new("/p/assets/vfx/a/b/deep.mp4")).is_none());
        assert!(classify_render_event(root, Path::new("/p/assets/vfx/a/project.aep")).is_none());
        assert!(classify_render_event(root, Path::new("/p/assets/vfx/a/.render.mp4.tmp")).is_none());
        assert!(classify_render_event(root, Path::new("/elsewhere/a/x.mp4")).is_none());
    }

    #[test]
    fn list_effects_scans_folders_and_sorts_renders() {
        let tmp = std::env::temp_dir().join(format!("vfx-test-{}", std::process::id()));
        let dir = tmp.join("assets/vfx/glitch");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("v1.mp4"), b"a").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        let effects = list_effects(&tmp);
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].effect, "glitch");
        assert_eq!(effects[0].renders, vec!["v1.mp4".to_string()]);
        std::fs::remove_dir_all(&tmp).ok();
    }
}
```

- [ ] **Step 2: Run to fail** — `cargo test -p katto vfx`.

- [ ] **Step 3: Implement** pure fns, then watcher + commands + broadcast + wiring
  (setup hook spawns `vfx::start_watch` next to `volumes::start_watcher`); regenerate
  bindings.

- [ ] **Step 4: Run** — `cargo test -p katto` PASS, clippy clean.

- [ ] **Step 5: Commit via `ship`** — paths:
  `src-tauri/src/vfx.rs src-tauri/src/commands/vfx.rs src-tauri/src/commands.rs src-tauri/src/broadcast.rs src-tauri/src/lib.rs src/lib/ipc/bindings.gen.ts` —
  `feat(vfx): effect scaffold + dock session + render folder watch`.

---

### Task 18: VFX frontend — effects card on project detail

**Files:**
- Create: `src/lib/ipc/vfx.ts`
- Create: `src/features/vfx/vfx-card.tsx` (+ test)
- Create: `src/features/vfx/new-effect-dialog.tsx`
- Modify: `src/features/projects/detail/project-detail.tsx` (**STASH PROCEDURE**)
- Modify: `src/lib/ipc/broadcast.ts`, `src/hooks/use-broadcast-invalidation.ts`
  (`vfxRenderLanded` → invalidate vfx keys)

**Interfaces:**
- Consumes: bindings `createVfxEffect`, `listVfxEffects`, `VfxEffect`,
  `vfxRenderLanded` event; `convertFileSrc` from `@tauri-apps/api/core` (asset
  protocol — the render preview `<video>` src); `useUiStore.openDock`;
  Card/Dialog/Button primitives; `ProjectDetail`'s existing card layout
  (`src/features/projects/detail/project-detail.tsx:87` — slot after `FootageCard`).
- Produces:
  - `src/lib/ipc/vfx.ts`: `vfxKeys = { byProject: (slug: string) => ["vfx", slug] as const }`,
    `listVfxEffects(slug)`, `createVfxEffect(slug, name): Promise<string>`.
  - `VfxCard({slug})`: header "Effects" (sans, card pattern shared by the sibling
    cards) + "New effect" secondary button → `NewEffectDialog` (single name field,
    submit → `createVfxEffect` → on success close dialog, invalidate, and
    `openDock(sessionId)`); body: empty state ("No effects yet. New effect opens a
    Claude session in its folder.") or a grid of effect tiles — effect name (sans) +
    latest render `<video muted playsInline preload="metadata">` via `convertFileSrc`
    when a render exists, else a quiet "no render yet" line; render count when > 1
    (`tabular-nums`).

- [ ] **Step 1: Failing component test** `vfx-card.test.tsx`: (a) empty state renders
  the invitation line; (b) with fixture effects, tiles render names + a video element
  for the one with renders (assert by role/test-id per project conventions — check how
  `footage-card.test.tsx` asserts media); (c) "New effect" → dialog → typing
  "Intro Glitch" + submit calls `createVfxEffect("proj", "Intro Glitch")` (mock IPC)
  and `openDock` with the returned id. Follow the existing project-detail test file's
  mocking style (it exists: `project-detail.test.tsx` — DO NOT edit that file; write a
  standalone `vfx-card.test.tsx`).

- [ ] **Step 2: Run to fail** — `bunx vitest run src/features/vfx`.

- [ ] **Step 3: Implement**; then apply the stash procedure and add `<VfxCard slug={slug}/>`
  to `project-detail.tsx` after `FootageCard`. Verify with
  `git diff src/features/projects/detail/project-detail.tsx` that your change is ONE
  clean hunk separate from the owner's DateInput hunks (the stash held those).

- [ ] **Step 4: Run** — `bunx vitest run src/features` PASS (including the existing
  project-detail tests — if the owner's uncommitted test hunks conflict with the new
  card, the stash keeps them out of the committed tree; run vitest AFTER the commit
  with the stash popped to confirm the working tree is also green).

- [ ] **Step 5: Commit via `ship`** (stash dance): stash owner paths → stage
  `src/lib/ipc/vfx.ts src/features/vfx/ src/features/projects/detail/project-detail.tsx src/lib/ipc/broadcast.ts src/hooks/use-broadcast-invalidation.ts`
  → verify `git diff --cached` has no DateInput hunk → commit
  `feat(vfx): effects card with render previews on project detail` → `git stash pop`.

---

### Task 19: Curation review treatment in the backlog

The notification deep-links to `katto://ideas` → Planner backlog. Curated ideas must
read as *suggestions awaiting a human*: rationale, suggested-kind provenance, lean
notch, source link. (D7: the human's keep/promote/discard stays exactly the existing
buttons — no new verdict UI.)

**Files:**
- Modify: `src/features/planner/backlog/backlog-view.tsx` (+ its test file)
- Possibly modify: `src/lib/ipc/ideas.ts` (only if `IdeaPatch` lacks `kind_source`)
- Possibly modify: `src-tauri/src/commands/ideas.rs` + `src-tauri/src/db/ideas.rs`
  (same condition; check `update` first)

**Interfaces:**
- Consumes: `Idea` fields already in bindings (`rationale`, `kindSource`, `kindWhy`,
  `evidenceJson`, `source`, `sourceUrl`, `type`), existing `updateIdea` mutation,
  `IdeaRow` component structure (read `backlog-view.tsx` first).
- Produces:
  - In `IdeaRow`, for ideas with `rationale`: the rationale as a muted single line
    under the title (sans, truncated with `title=` full text).
  - Kind select: when `kindSource === "ai"`, a quiet "suggested" affix next to the
    select (one chip, no second encoding) + `kindWhy` as its tooltip; the human
    changing OR confirming the kind patches `{kind, kind_source: "human"}` (confirm =
    re-selecting the same value; add a small check-button "keep suggestion" if the
    select's onChange can't express confirmation — decide by the existing select
    primitive's behavior).
  - Lean notch: parse `evidenceJson` (safe `JSON.parse` in a tiny pure helper
    `parseLean(evidenceJson: string | null): "hold" | "lean" | "strong" | null` in
    `src/features/planner/backlog/model/lean.ts` + test) → a 3-step vertical notch
    (three 3px bars, filled 1/2/3, `--ember` fill, `--border` empty) with
    `aria-label="lean: strong"`. Never a number.
  - Source: when `sourceUrl` exists, the domain as a small external link (opener via
    the existing shell command pattern — check how other external links open,
    `commands/shell.rs` exists).

- [ ] **Step 1: Failing tests** — `model/lean.test.ts` (null on garbage/absent; parses
  the three values; ignores unknown strings → null) + extend the backlog view test:
  fixture idea with `rationale`/`kindSource: "ai"`/`evidenceJson: '{"lean":"strong"}'`
  renders the rationale text, the "suggested" marker, and `aria-label="lean: strong"`;
  changing kind fires `updateIdea` with `kind_source: "human"` (adjust to the real
  patch field naming in bindings).

- [ ] **Step 2: Run to fail** — `bunx vitest run src/features/planner`.

- [ ] **Step 3: Implement**; if the backend `IdeaPatch` lacks `kind_source`, add it
  (repo `update` SQL + command struct + bindings regen — small, same-commit).

- [ ] **Step 4: Run** — `bunx vitest run src/features/planner` + `cargo test -p katto ideas`
  PASS.

- [ ] **Step 5: Commit via `ship`** — paths:
  `src/features/planner/backlog/ src/lib/ipc/ideas.ts` (+ backend files + bindings if
  touched) — `feat(planner): curation provenance — rationale, suggested kind, lean notch`.

---

### Task 20: Owner checklist + full gate

**Files:**
- Modify: `docs/overnight-run.md` (untracked — never staged)

- [ ] **Step 1: Append a "Phase 6 — owner verification" section** to
  `docs/overnight-run.md`, matching the file's existing checkbox style, covering at
  minimum:
  - [ ] Dock walkthrough: sidebar Claude icon → panel slides over; "New session"
    opens a live claude tab; type into it mid-run; hide the panel while it works
    (icon pulses); reopen — scrollback intact.
  - [ ] Icon states: running pulse, needs-input badge (trigger a permission prompt),
    done check flash, notification arrives when the panel is hidden and its click
    opens the dock.
  - [ ] Cut plan via dock: "Plan rough cut" on a project with footage → session tab
    "cut plan: …" visibly works → cuts.json lands → pipeline continues; toggle
    "Cut planning in the dock" off → old subprocess path still works.
  - [ ] Idle reaping: leave a session idle past the timeout (set 2 min in Settings)
    → tab shows "closed after idle"; a focused panel session is spared.
  - [ ] Nightly curation dry run: Settings → "Run now" → session judges the delta →
    notification "kept N / discarded M" → click lands on the backlog; new ideas
    show rationale + suggested kind + lean notch; confirm a kind (provenance flips).
  - [ ] Scheduler catch-up: set curation to a time while the Mac will sleep, sleep
    through it, wake → runs once (check events log); does not run again the same day.
  - [ ] VFX: "New effect" on a project → session opens in `assets/vfx/<slug>/`;
    drop/render an .mp4 there → it appears on the project card + events row.
  - [ ] Real SD-card checkpoint from phases 3–5 remains listed (don't duplicate).
- [ ] **Step 2: Run the full gate** — `just check` from the workspace root. Expected:
  every step green. Paste the tail of the output when reporting phase completion.
- [ ] **Step 3: Verify tree hygiene** — `git status`: only owner DateInput files +
  untracked docs remain dirty; no plan/doc/CLAUDE.md staged anywhere in the branch
  history (`git log --stat -20` spot-check).
- [ ] **Step 4:** No commit (checklist file is untracked). Phase 6 done — report with
  the gate tail + the checklist location.

---

## Self-review notes (performed at write time)

- **PRD scope → tasks:** session pool (6), states + hooks (2/5/6), push API =
  `write_session` (8), idle reaping (7), dock UI + icon states (9), PTY→UI streaming
  (6/8/10), terminal panel (9/10), scheduler + wake (11/12), nightly curation +
  discovery toggle + deep-linked notification (13/14), cut-plan re-route + fallback
  (15), VFX cockpit + render watch (17/18), settings + palette (16), curation review
  surface (19), error-handling rows (each task's events rows), testing matrix (pure:
  2/3/4/7/11/15/17; integration: 5/6; frontend: 9/10/16/18/19; manual: 20). Out of
  scope honored: no transcript persistence beyond scrollback, no remote sessions, no
  browser/thumbnails work.
- **Deviations from the PRD's letter, with reason:** (a) hook POST body carries katto's
  session id in the URL, not `{session, event}` in the body — same information, no
  claude-side templating needed; (b) curation summary computed from DB deltas instead
  of parsing session output — strictly more robust, PRD's "tolerates absent summary"
  satisfied trivially; (c) "done" is a UI flash on `running→idle`, not a persisted
  fifth state — the PRD's dock-icon "brief check (done)" is inherently transient;
  (d) `--append-system-prompt` is used for the cut-decider prompt (PRD names the flag
  for sessions generally); (e) `create_vfx_effect` returns the spawned `session_id`
  rather than the PRD wiring table's `path` — the caller needs the session to focus the
  dock, and the path is derivable (`<root>/assets/vfx/<slug>`).
- **Cross-task name consistency checked:** `SessionState` serde tags (`kind`,
  snake_case) ↔ Task 9 frontend tests (with an explicit re-verify note against
  bindings); `SessionTask` public vs internal fields (Task 8 note); `watch_first_stop`
  produced in 14's pool extension and consumed in 15; `CloseReason::IdleReaped` ↔
  `tabNote`; settings keys named identically in 14/15/16.
- **Known [CHECK] items for the implementer:** permission-rule matcher syntax (iam
  docs), objc2-user-notifications delegate item names, objc2-foundation feature names,
  specta's generated field casing in `bindings.gen.ts`, the engine cuts fixture name in
  Task 15, `db/projects.rs` accessor name in Task 17.

