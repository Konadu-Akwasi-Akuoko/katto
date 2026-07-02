# Phase 6 — Claude Dock & Automations

## Goal

Every AI task becomes a visible, interruptible Claude Code session in a dock panel; katto
gains a scheduler with anacron catch-up semantics; the first automation (nightly idea
curation) and the VFX cockpit go live; cut planning re-routes through the dock.

## Why this order

The dock needs real work to display — the pipeline (4/5) and planner data (2) provide it.
Everything here is additive orchestration over existing features (D15: choreography, never
intelligence).

## User stories

- I click "Plan rough cut" and watch the actual Claude session do it in a dock tab; I can type
  into that session to redirect it mid-run.
- Overnight, katto ran discovery + curation; the notification says "3 ideas kept"; clicking it
  lands me on the Ideas page. It ran even though the Mac was asleep at 00:00.
- I hit "New effect" on a project; a session opens already sitting in
  `assets/vfx/intro-glitch/` with the HyperFrames toolchain; the render appears on the project
  card when it lands.
- A session that needs my input badges the dock icon and notifies me if the panel is hidden.
- Idle sessions close themselves after 5 minutes, leaving a quiet "closed after idle" note.

## Scope with acceptance criteria

| Feature | Acceptance criteria |
|---|---|
| Session pool | PTY-backed real `claude` sessions (portable-pty 0.9): spawn with cwd, initial prompt, optional `--append-system-prompt`, env forwarded via `zsh -lc`, `TERM=xterm-256color`; one tab per task, labeled ("cut plan: <video>", "ideas: nightly", "vfx: <effect>"); pool scales on demand — busy pool + new task = new session |
| Session states | `idle / running / needs-input / done / failed` per session. Primary signal: spawned sessions get a katto-generated `--settings` JSON whose `Stop` + `Notification` hooks POST `{session, event}` to katto's localhost endpoint (127.0.0.1, random port, token-authed). Fallback heuristic: output silence + prompt patterns. Failed task keeps its tab open with the error (D18) |
| Push API | `push_to_session(id, text)` writes to PTY stdin — the app and the keyboard share the same seat; no hidden side-channel |
| Idle reaping | Sessions idle > setting (default 5 min; options 2/10) are closed with a "closed after idle" note in the tab strip + events row; a session whose panel is open and focused is exempt |
| Dock UI | Sidebar Claude icon with 4 states: static (idle) / pulsing ring (running) / attention badge + notification-if-hidden (needs-input) / brief check (done). Click = slide-over panel: tab strip + xterm.js terminal (`@xterm/xterm` 6, fit addon + webgl); click again hides while work continues |
| PTY→UI streaming | Reader thread per session batching ~16 ms/16 KB flushes into `tauri::ipc::Channel` (events are too slow for PTY volume); xterm writes raw bytes; resize propagates FitAddon → `master.resize()` |
| Scheduler | 60 s `tokio::time::interval` tick evaluating `scheduled_jobs`: due iff now past today's slot AND `last_success_at` older than the spec's period − grace; late ticks after sleep fire naturally; `objc2-app-kit` `NSWorkspace` `didWakeNotification` observer triggers an immediate evaluation on wake; a missed slot runs **once**, never piles up |
| Nightly curation | `scheduled_jobs['nightly-curation']` (daily 00:00, catch-up N=20 h): optionally runs `studio-discover --db <katto.db>` (settings toggle; requires `uv` + the hyper-frames checkout path configured), then opens a dock session with the curation prompt (contract below) against katto's DB; on completion: notification "curation: kept N / discarded M" deep-linking `katto://ideas`; new ideas appear with status `backlog` |
| Cut-plan re-route | In-app "Plan rough cut" now runs the planning stage as a dock session (visible); the session writes `cuts.json` into the bundle; katto watches for the file, validates with the Phase-4 validators (retry contract unchanged), continues the pipeline. The Phase-4 subprocess planner remains the CLI path and the fallback when the dock is disabled |
| VFX cockpit | Project detail "New effect" → name → scaffolds `<project>/assets/vfx/<effect>/` → dock session opens there (the owner's global Claude setup provides HyperFrames/Remotion tooling; katto adds nothing); `notify`-based folder watch on `assets/vfx/` surfaces new `.mp4/.mov` renders on the project card + events row |
| Settings + palette | Idle-reap timeout; curation schedule on/off + time; discovery toggle + hyper-frames path; palette: "new claude session", "open dock", "run nightly curation now" |

## Curation contract (embedded; pattern source: the owner's studio-ideas skill)

The dock session prompt instructs Claude to, against katto's SQLite DB:

1. Read **only the unjudged delta**: rows `WHERE judged_at IS NULL` from `raw_signal`
   (aggregator/video rows and `youtube-comments:*` rows batched separately; never the whole
   table).
2. Novelty guard: existing `ideas` titles (status `backlog`/`promoted`) + project folder list.
3. Judge each row **keep or discard** — binary, qualitative, with a one-line *why-pursue*
   rationale. **Never a number, score, rank, percentage, or grade** (D7). Lenses: fit
   (on-brand), novelty, demand shape (recurring comment asks = strong keep, capture quotes).
4. Insert keepers into `ideas`: `id = lower(hex(randomblob(8)))`, `status='backlog'`,
   `type` mapped from source (`youtube-comments:*`→`comment_demand`, `youtube:*`→`mirror`,
   `hn`/`reddit:*`/`lobsters`/`dailydev`→`trend`), suggested `kind` with `kind_source='ai'`
   + one-line `kind_why`, `evidence_json` with categorical `lean` (`hold|lean|strong` — a
   meter hint, not a grade) + source metrics.
5. Mark **every** row read this pass: `judged_at = datetime('now')`,
   `judged_verdict = 'kept'|'discarded'`; discarded rows stay as audit trail, never re-judged.
6. Reply with a compact summary (kept N by type, discarded M, titles + suggested kind).

Housekeeping (katto-side, not the session): prune judged `raw_signal` older than 90 days.

## Backend (Rust)

New modules: `sessions.rs` + `sessions/` (`pool.rs`, `pty.rs`, `state.rs` state machine,
`hooks_endpoint.rs` — tiny hyper/axum listener on 127.0.0.1, `reap.rs`), `scheduler.rs`
(tick + due math as pure functions + wake observer), `curation.rs` (prompt assembly + result
notification), `vfx.rs` (scaffold + render watch), `commands/{sessions,scheduler,vfx}.rs`.

Crates added: `portable-pty 0.9`, `axum` (or `tiny_http`) for the hooks endpoint,
`objc2-app-kit 0.3` (wake notification), `@xterm/xterm 6.0` + `@xterm/addon-fit` +
`@xterm/addon-webgl` (frontend).

## Frontend (React)

`src/features/dock/`: panel (slide-over), tab strip (state chips, close, "closed after idle"
notes), `terminal.tsx` (xterm lifecycle: open→fit→stream→resize→dispose), dock icon states in
the sidebar; `src/features/vfx/` card section on project detail (effects grid + latest render
preview via asset protocol).

## Wiring / IPC

| Command | Notes |
|---|---|
| `spawn_session(task: SessionTask) -> session_id` | `SessionTask {label, cwd, initial_prompt?, append_system_prompt?}` |
| `attach_session(id, on_data: Channel<Vec<u8>>) -> ()` | PTY byte stream (batched) |
| `write_session(id, data: String)` / `resize_session(id, cols, rows)` | keyboard + fit |
| `close_session(id)` / `list_sessions() -> Vec<SessionInfo>` | `SessionInfo {id, label, state, cwd, started_at, idle_since?}` |
| `run_scheduled_job_now(name)` / `get_scheduler_state() -> Vec<ScheduledJobInfo>` | settings surface |
| `create_vfx_effect(slug, name) -> path` | scaffold + session |
| Broadcast | `session-state-changed {id, state}`, `vfx-render-landed {slug, effect, file}` |

## Data-model deltas

Migration: seed `scheduled_jobs` row `('nightly-curation', 'daily@00:00;catchup=20h', NULL, 1)`.
Session metadata is runtime-only (events row per spawn/close/fail).

## Error handling

- `claude` not on PATH → spawn fails typed; dock shows install hint; scheduled curation skips
  with an events row (never a crash loop).
- Hooks endpoint unreachable from session (sandbox/settings issue) → state machine degrades
  to the output heuristic; noted once in events.
- PTY died unexpectedly → session `failed`, tab keeps scrollback + exit status.
- Discovery run fails (uv missing, path wrong) → curation proceeds without it, failure noted
  in the notification body.
- Curation session writes invalid rows → constraint failures surface in the session output
  itself (visible by design); katto's summary parser tolerates absent summary.
- Scheduler never runs a job twice for one slot: due-evaluation is idempotent on
  `last_success_at`, which is written only on success; failures retry at the next tick with
  backoff cap (1 h).

## Testing

- Pure: scheduler due-math table (slot passed while asleep → one run; multiple missed days →
  one run; success updates gate; failure retries with cap), idle-reap decision, session state
  machine transitions (fake PTY events + fake hook POSTs).
- Integration: pool spawn/write/read against a fake shell (`bash -c 'cat'`) — not `claude` —
  asserting stream batching and resize; hooks endpoint auth (bad token rejected).
- Frontend: dock tab state rendering; terminal lifecycle with a scripted byte stream.
- Manual checkpoints: real `claude` session in the dock (type into it mid-task); sleep the
  Mac through 00:00 → wake → curation runs once; notification deep-link lands on Ideas.

## Out of scope

Multi-machine/remote sessions, session transcript persistence beyond scrollback, curation
sources beyond the studio-discover set, the browser/thumbnails/import (Phase 7).

## Exit criteria

A cut plan runs visibly in a dock tab (interruptible, typeable); the nightly job survives a
sleep/catch-up cycle, notifies, and deep-links; `just check` green.
