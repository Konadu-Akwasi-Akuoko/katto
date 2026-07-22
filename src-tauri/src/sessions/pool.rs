use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use portable_pty::{ChildKiller, MasterPty, PtySize};
use tauri::{AppHandle, Manager};

use crate::error::{Error, Result};
use crate::sessions::buffer::Scrollback;
use crate::sessions::hooks_endpoint::{self, HookEvent, HooksEndpoint};
use crate::sessions::launch::{LaunchSpec, hook_settings_json, shell_invocation};
use crate::sessions::pty::spawn_pty;
use crate::sessions::state::{CloseReason, SessionEvent, SessionState, apply};
use crate::sessions::{Program, SessionInfo, SessionTask};

const SCROLLBACK_CAP: usize = 2 * 1024 * 1024;
const FLUSH_BYTES: usize = 16 * 1024;
const FLUSH_INTERVAL: Duration = Duration::from_millis(16);
/// Degraded-mode heuristic: a hooks-silent session Running with no output for
/// this long is treated as idle.
pub(crate) const SILENCE_TIMEOUT: Duration = Duration::from_secs(45);
const SPAWN_COLS: u16 = 120;
const SPAWN_ROWS: u16 = 30;

type Sink = Box<dyn Fn(&[u8]) -> bool + Send>;
type DoneTx = tokio::sync::oneshot::Sender<std::result::Result<(), String>>;

/// The session pool: owns every live PTY, its scrollback, and its state. All
/// app-dependent side effects (jobs/events/broadcast/notification) no-op until
/// `start(app)` runs, so the pool is fully testable headless.
#[derive(Clone)]
pub struct SessionPool {
    inner: Arc<Inner>,
}

struct Inner {
    sessions: Mutex<HashMap<String, Entry>>,
    endpoint: Mutex<Option<HooksEndpoint>>,
    dock: Mutex<DockFocus>,
    app: OnceLock<AppHandle>,
}

#[derive(Default)]
struct DockFocus {
    open: bool,
    focused: Option<String>,
}

struct Entry {
    label: String,
    cwd: PathBuf,
    state: SessionState,
    hooks_live: bool,
    degraded_reported: bool,
    buffer: Scrollback,
    writer: Option<Box<dyn Write + Send>>,
    master: Option<Box<dyn MasterPty + Send>>,
    killer: Option<Box<dyn ChildKiller + Send + Sync>>,
    sink: Option<Sink>,
    started_at: String,
    idle_since: Option<Instant>,
    last_output: Instant,
    job_done: Option<DoneTx>,
    settings_path: Option<PathBuf>,
}

/// A state change collected under the sessions lock; side effects run after
/// the guard is dropped.
struct Transition {
    id: String,
    label: String,
    state: SessionState,
}

fn set_state(entry: &mut Entry, id: &str, new: SessionState) -> Transition {
    if matches!(new, SessionState::Idle) {
        if !matches!(entry.state, SessionState::Idle) {
            entry.idle_since = Some(Instant::now());
        }
    } else {
        entry.idle_since = None;
    }
    entry.state = new.clone();
    Transition {
        id: id.to_string(),
        label: entry.label.clone(),
        state: new,
    }
}

impl Default for SessionPool {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionPool {
    pub fn new() -> Self {
        SessionPool {
            inner: Arc::new(Inner {
                sessions: Mutex::new(HashMap::new()),
                endpoint: Mutex::new(None),
                dock: Mutex::new(DockFocus::default()),
                app: OnceLock::new(),
            }),
        }
    }

    fn sessions(&self) -> MutexGuard<'_, HashMap<String, Entry>> {
        self.inner
            .sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Wire the app handle and start the hooks endpoint plus the degraded-mode
    /// silence timer. Called once from the setup hook.
    pub fn start(&self, app: AppHandle) -> Result<()> {
        let _ = self.inner.app.set(app);
        self.ensure_endpoint()?;
        let weak = Arc::downgrade(&self.inner);
        tauri::async_runtime::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(15));
            loop {
                tick.tick().await;
                let Some(inner) = weak.upgrade() else { break };
                SessionPool { inner }.apply_silence_timeouts();
            }
        });
        Ok(())
    }

    /// Start the hooks endpoint (idempotent) and return its `(port, token)`.
    /// The pump thread translates deliveries into state-machine events.
    fn ensure_endpoint(&self) -> Result<(u16, String)> {
        let mut guard = self
            .inner
            .endpoint
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(ep) = guard.as_ref() {
            return Ok((ep.port, ep.token.clone()));
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let ep = hooks_endpoint::start(tx)?;
        let port_token = (ep.port, ep.token.clone());
        *guard = Some(ep);
        drop(guard);

        let weak = Arc::downgrade(&self.inner);
        std::thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                let Some(inner) = weak.upgrade() else { break };
                SessionPool { inner }.apply_hook_event(event);
            }
        });
        Ok(port_token)
    }

    /// Spawn a full app-integrated session: jobs row (`claude_session`),
    /// events row, broadcast. `Program::Claude` resolves the CLI, writes the
    /// per-session hooks settings file (0600), and launches through `zsh -lc`
    /// so the owner's login environment (PATH, subscription auth) applies.
    pub async fn spawn(
        &self,
        app: &AppHandle,
        task: SessionTask,
        program: Program,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let (bin, args, settings_path) = match &program {
            Program::Custom(cmdline) => (
                "/bin/sh".to_string(),
                vec!["-c".to_string(), cmdline.clone()],
                None,
            ),
            Program::Claude => {
                let state = app.state::<crate::state::AppState>();
                let configured = state
                    .db
                    .call(|conn| crate::db::settings::get(conn, "claude_path"))
                    .await?
                    .filter(|p| !p.is_empty())
                    .map(PathBuf::from);
                let claude_path = configured
                    .or_else(katto_engine::detect::detect_claude)
                    .ok_or_else(|| {
                        Error::ClaudeMissing(
                            "claude CLI not found — install it or set its path in Settings"
                                .to_string(),
                        )
                    })?;
                let dir = app
                    .path()
                    .app_data_dir()
                    .map_err(|err| Error::SessionSpawn(err.to_string()))?
                    .join("sessions");
                std::fs::create_dir_all(&dir)?;
                let (port, token) = self.ensure_endpoint()?;
                let settings_path = dir.join(format!("{id}.settings.json"));
                let json = hook_settings_json(port, &token, &id, &task.permission_allow);
                std::fs::write(&settings_path, json)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(
                        &settings_path,
                        std::fs::Permissions::from_mode(0o600),
                    )?;
                }
                let spec = LaunchSpec {
                    claude_path,
                    cwd: task.cwd.clone(),
                    initial_prompt: task.initial_prompt.clone(),
                    append_system_prompt: task.append_system_prompt.clone(),
                    settings_path: settings_path.clone(),
                    permission_mode: task.permission_mode.clone(),
                };
                (
                    "zsh".to_string(),
                    vec!["-lc".to_string(), shell_invocation(&spec)],
                    Some(settings_path),
                )
            }
        };

        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        {
            let state = app.state::<crate::state::AppState>();
            state
                .jobs
                .spawn(
                    "claude_session",
                    &task.label,
                    Some(serde_json::json!({ "session_id": id }).to_string()),
                    move |_ctx| async move {
                        done_rx.await.unwrap_or(Err("session dropped".to_string()))
                    },
                )
                .await?;
        }

        let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
        if let Err(err) = self.spawn_entry(
            &id,
            &task,
            &bin,
            &args_ref,
            settings_path.clone(),
            Some(done_tx),
        ) {
            if let Some(path) = settings_path {
                let _ = std::fs::remove_file(path);
            }
            return Err(err);
        }

        self.record_event(
            "session_spawned",
            serde_json::json!({
                "session_id": id,
                "label": task.label,
                "cwd": task.cwd.to_string_lossy(),
            }),
        );
        self.emit_sessions_changed();
        Ok(id)
    }

    /// Test seam: spawn minus jobs/events/broadcast. Only `Program::Custom`
    /// makes sense headless — `Claude` needs the app context.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) async fn spawn_headless(
        &self,
        task: SessionTask,
        program: Program,
    ) -> Result<String> {
        let Program::Custom(cmdline) = program else {
            return Err(Error::SessionSpawn(
                "claude sessions require the app context".to_string(),
            ));
        };
        let id = uuid::Uuid::new_v4().to_string();
        self.spawn_entry(&id, &task, "/bin/sh", &["-c", &cmdline], None, None)?;
        Ok(id)
    }

    /// Spawn the PTY, insert the entry, and start the three per-session
    /// threads (reader, flusher, waiter).
    fn spawn_entry(
        &self,
        id: &str,
        task: &SessionTask,
        bin: &str,
        args: &[&str],
        settings_path: Option<PathBuf>,
        job_done: Option<DoneTx>,
    ) -> Result<()> {
        let handle = spawn_pty(bin, args, &task.cwd, SPAWN_COLS, SPAWN_ROWS)?;
        let started_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let entry = Entry {
            label: task.label.clone(),
            cwd: task.cwd.clone(),
            state: SessionState::Running,
            hooks_live: false,
            degraded_reported: false,
            buffer: Scrollback::new(SCROLLBACK_CAP),
            writer: Some(handle.writer),
            master: Some(handle.master),
            killer: Some(handle.killer),
            sink: None,
            started_at,
            idle_since: None,
            last_output: Instant::now(),
            job_done,
            settings_path,
        };
        self.sessions().insert(id.to_string(), entry);

        let (chunk_tx, chunk_rx) = std::sync::mpsc::channel::<Vec<u8>>();
        let mut reader = handle.reader;
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if chunk_tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        let weak = Arc::downgrade(&self.inner);
        let flush_id = id.to_string();
        std::thread::spawn(move || {
            let mut pending: Vec<u8> = Vec::new();
            let flush = |pending: &mut Vec<u8>| -> bool {
                if pending.is_empty() {
                    return true;
                }
                let Some(inner) = weak.upgrade() else {
                    return false;
                };
                SessionPool { inner }.ingest_output(&flush_id, pending);
                pending.clear();
                true
            };
            loop {
                match chunk_rx.recv_timeout(FLUSH_INTERVAL) {
                    Ok(chunk) => {
                        pending.extend(chunk);
                        if pending.len() >= FLUSH_BYTES && !flush(&mut pending) {
                            break;
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        if !flush(&mut pending) {
                            break;
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        flush(&mut pending);
                        break;
                    }
                }
            }
        });

        let weak = Arc::downgrade(&self.inner);
        let wait_id = id.to_string();
        let mut child = handle.child;
        std::thread::spawn(move || {
            let code = child.wait().ok().map(|status| status.exit_code());
            if let Some(inner) = weak.upgrade() {
                SessionPool { inner }.on_exit(&wait_id, code);
            }
        });

        Ok(())
    }

    /// Push a flushed output batch into scrollback + the attached sink, and
    /// feed the degraded-mode burst heuristic.
    fn ingest_output(&self, id: &str, bytes: &[u8]) {
        let mut transition = None;
        {
            let mut sessions = self.sessions();
            let Some(entry) = sessions.get_mut(id) else {
                return;
            };
            entry.buffer.push(bytes);
            entry.last_output = Instant::now();
            if let Some(sink) = &entry.sink
                && !sink(bytes)
            {
                entry.sink = None;
            }
            if !entry.hooks_live {
                let event = SessionEvent::OutputBytes { len: bytes.len() };
                let new = apply(&entry.state, &event, false);
                if new != entry.state {
                    transition = Some(set_state(entry, id, new));
                }
            }
        }
        if let Some(t) = transition {
            self.after_transition(t);
        }
    }

    /// A hook delivery proves the endpoint pipeline works for this session.
    pub(crate) fn apply_hook_event(&self, event: HookEvent) {
        let (id, machine_event) = match event {
            HookEvent::Stop { session_id } => (session_id, SessionEvent::HookStop),
            HookEvent::Notification { session_id } => (session_id, SessionEvent::HookNotification),
        };
        let mut transition = None;
        {
            let mut sessions = self.sessions();
            let Some(entry) = sessions.get_mut(&id) else {
                return;
            };
            entry.hooks_live = true;
            let new = apply(&entry.state, &machine_event, true);
            if new != entry.state {
                transition = Some(set_state(entry, &id, new));
            }
        }
        if let Some(t) = transition {
            self.after_transition(t);
        }
    }

    /// Degraded-mode timer: sessions the endpoint never heard from go idle
    /// after 45 s of silence; the first such transition per session records a
    /// `session_hooks_degraded` events row (once, D18).
    fn apply_silence_timeouts(&self) {
        let mut transitions = Vec::new();
        let mut degraded = Vec::new();
        {
            let mut sessions = self.sessions();
            for (id, entry) in sessions.iter_mut() {
                if entry.hooks_live || entry.last_output.elapsed() < SILENCE_TIMEOUT {
                    continue;
                }
                let new = apply(&entry.state, &SessionEvent::SilenceTimeout, false);
                if new != entry.state {
                    if !entry.degraded_reported {
                        entry.degraded_reported = true;
                        degraded.push((id.clone(), entry.label.clone()));
                    }
                    transitions.push(set_state(entry, id, new));
                }
            }
        }
        for (id, label) in degraded {
            self.record_event(
                "session_hooks_degraded",
                serde_json::json!({ "session_id": id, "label": label }),
            );
        }
        for t in transitions {
            self.after_transition(t);
        }
    }

    /// PTY child exited: apply the terminal transition, resolve the job, and
    /// clean up the settings file. `close` wins any race — a session already
    /// terminal stays exactly as it is.
    fn on_exit(&self, id: &str, code: Option<u32>) {
        let (transition, job_done, settings) = {
            let mut sessions = self.sessions();
            let Some(entry) = sessions.get_mut(id) else {
                return;
            };
            let new = apply(
                &entry.state,
                &SessionEvent::PtyExited { code },
                entry.hooks_live,
            );
            let transition = (new != entry.state).then(|| set_state(entry, id, new.clone()));
            let job_done = entry.job_done.take();
            let settings = entry.settings_path.take();
            entry.writer = None;
            entry.master = None;
            entry.killer = None;
            (transition, job_done, settings)
        };
        if let Some(path) = settings {
            let _ = std::fs::remove_file(path);
        }
        if let Some(tx) = job_done {
            let outcome = match self.get_state(id) {
                Some(SessionState::Failed { error }) => Err(error),
                _ => Ok(()),
            };
            let _ = tx.send(outcome);
        }
        if let Some(t) = transition {
            self.after_transition(t);
        }
        self.emit_sessions_changed();
    }

    /// Forward user keystrokes to the PTY; typing into an idle or needs-input
    /// session marks it running again.
    pub fn write(&self, id: &str, data: &[u8]) -> Result<()> {
        let mut transition = None;
        {
            let mut sessions = self.sessions();
            let entry = sessions
                .get_mut(id)
                .ok_or_else(|| Error::SessionNotFound(format!("no session {id}")))?;
            let writer = entry
                .writer
                .as_mut()
                .ok_or_else(|| Error::SessionNotFound(format!("session {id} is not live")))?;
            writer.write_all(data)?;
            let new = apply(&entry.state, &SessionEvent::UserInput, entry.hooks_live);
            if new != entry.state {
                transition = Some(set_state(entry, id, new));
            }
        }
        if let Some(t) = transition {
            self.after_transition(t);
        }
        Ok(())
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<()> {
        let sessions = self.sessions();
        let entry = sessions
            .get(id)
            .ok_or_else(|| Error::SessionNotFound(format!("no session {id}")))?;
        let master = entry
            .master
            .as_ref()
            .ok_or_else(|| Error::SessionNotFound(format!("session {id} is not live")))?;
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|err| Error::SessionSpawn(format!("pty resize failed: {err}")))?;
        Ok(())
    }

    /// Attach an output sink: the scrollback snapshot is replayed as the first
    /// send, then live batches follow. One sink per session — latest wins
    /// (there is one dock panel).
    #[cfg_attr(not(test), allow(dead_code))] // public `attach` wraps this from Task 8 on
    pub(crate) fn attach_sink(&self, id: &str, sink: Sink) {
        let mut sessions = self.sessions();
        let Some(entry) = sessions.get_mut(id) else {
            return;
        };
        let replay = entry.buffer.snapshot();
        if !replay.is_empty() && !sink(&replay) {
            return;
        }
        entry.sink = Some(sink);
    }

    /// Close a session: state to `Closed{reason}` first (winning the race with
    /// the waiter thread), then drop the writer and kill the child.
    pub async fn close(&self, id: &str, reason: CloseReason) -> Result<()> {
        let (transition, job_done, settings, killer) = {
            let mut sessions = self.sessions();
            let entry = sessions
                .get_mut(id)
                .ok_or_else(|| Error::SessionNotFound(format!("no session {id}")))?;
            let live = !matches!(
                entry.state,
                SessionState::Failed { .. } | SessionState::Closed { .. }
            );
            let transition = live.then(|| {
                set_state(
                    entry,
                    id,
                    SessionState::Closed {
                        reason: reason.clone(),
                    },
                )
            });
            let job_done = if live { entry.job_done.take() } else { None };
            let settings = entry.settings_path.take();
            let killer = entry.killer.take();
            entry.writer = None;
            (transition, job_done, settings, killer)
        };
        if let Some(mut k) = killer {
            let _ = k.kill();
        }
        if let Some(path) = settings {
            let _ = std::fs::remove_file(path);
        }
        if let Some(tx) = job_done {
            let _ = tx.send(Ok(()));
        }
        if matches!(reason, CloseReason::IdleReaped) {
            let label = self
                .sessions()
                .get(id)
                .map(|entry| entry.label.clone())
                .unwrap_or_default();
            self.record_event(
                "session_reaped",
                serde_json::json!({ "session_id": id, "label": label }),
            );
        }
        if let Some(t) = transition {
            self.after_transition(t);
        }
        self.emit_sessions_changed();
        Ok(())
    }

    pub fn list(&self) -> Vec<SessionInfo> {
        let sessions = self.sessions();
        let mut infos: Vec<SessionInfo> = sessions
            .iter()
            .map(|(id, entry)| SessionInfo {
                id: id.clone(),
                label: entry.label.clone(),
                state: entry.state.clone(),
                cwd: entry.cwd.to_string_lossy().into_owned(),
                started_at: entry.started_at.clone(),
                idle_since_secs: entry.idle_since.map(|at| at.elapsed().as_secs()),
            })
            .collect();
        infos.sort_by(|a, b| a.started_at.cmp(&b.started_at).then(a.id.cmp(&b.id)));
        infos
    }

    /// Reap exemption + needs-input notification suppression follow the dock
    /// panel's visibility, reported by the frontend.
    pub fn set_dock_focus(&self, open: bool, focused: Option<String>) {
        let mut dock = self
            .inner
            .dock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        dock.open = open;
        dock.focused = focused;
    }

    pub(crate) fn get_state(&self, id: &str) -> Option<SessionState> {
        self.sessions().get(id).map(|entry| entry.state.clone())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn scrollback(&self, id: &str) -> Option<Vec<u8>> {
        self.sessions().get(id).map(|entry| entry.buffer.snapshot())
    }

    fn after_transition(&self, t: Transition) {
        self.emit_state_changed(&t.id, &t.state);
        if matches!(t.state, SessionState::NeedsInput) {
            let panel_hidden = {
                let dock = self
                    .inner
                    .dock
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                !dock.open
            };
            if panel_hidden {
                self.notify_needs_input(&t.id, &t.label);
            }
        }
    }

    /// Broadcast stub — wired to tauri-specta events with the command layer
    /// (Task 8); headless pools stay silent.
    fn emit_state_changed(&self, _id: &str, _state: &SessionState) {}

    /// Broadcast stub — see [`Self::emit_state_changed`].
    fn emit_sessions_changed(&self) {}

    fn notify_needs_input(&self, id: &str, label: &str) {
        let Some(app) = self.inner.app.get() else {
            return;
        };
        let _ = crate::notify::notify(
            app,
            "Claude needs input",
            &format!("{label} is waiting on you"),
            "katto://dock",
        );
        self.record_event(
            "session_needs_input",
            serde_json::json!({ "session_id": id, "label": label }),
        );
    }

    /// Best-effort events row; no-ops headless (tests), never blocks a caller.
    fn record_event(&self, kind: &'static str, payload: serde_json::Value) {
        let Some(app) = self.inner.app.get() else {
            return;
        };
        let Some(state) = app.try_state::<crate::state::AppState>() else {
            return;
        };
        let db = state.db.clone();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let payload = payload.to_string();
            let _ = db
                .call(move |conn| crate::db::events::record(conn, kind, None, Some(&payload)))
                .await;
            crate::broadcast::events_appended(&app);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::state::{CloseReason, SessionState};
    use crate::sessions::{Program, SessionTask};
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
        let id = pool
            .spawn_headless(task("t"), Program::Custom("bash -c 'cat'".into()))
            .await
            .unwrap();
        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        pool.attach_sink(&id, Box::new(move |bytes| tx.send(bytes.to_vec()).is_ok()));
        pool.write(&id, b"hello\n").unwrap();
        let mut got = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline
            && !String::from_utf8_lossy(&got).contains("hello")
        {
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
        let id = pool
            .spawn_headless(
                task("t"),
                Program::Custom("bash -c 'echo pre-attach; cat'".into()),
            )
            .await
            .unwrap();
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
        let id = pool
            .spawn_headless(task("t"), Program::Custom("bash -c 'exit 0'".into()))
            .await
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if matches!(pool.get_state(&id), Some(SessionState::Closed { .. })) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(matches!(
            pool.get_state(&id),
            Some(SessionState::Closed {
                reason: CloseReason::Exited
            })
        ));
    }

    #[tokio::test]
    async fn dirty_exit_transitions_to_failed_and_keeps_scrollback() {
        let pool = SessionPool::new();
        let id = pool
            .spawn_headless(
                task("t"),
                Program::Custom("bash -c 'echo boom >&2; exit 3'".into()),
            )
            .await
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if matches!(pool.get_state(&id), Some(SessionState::Failed { .. })) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        match pool.get_state(&id) {
            Some(SessionState::Failed { error }) => assert!(error.contains('3')),
            other => panic!("expected Failed, got {other:?}"),
        }
        assert!(!pool.scrollback(&id).unwrap().is_empty());
    }

    /// Live smoke: spawns the owner's real `claude` (subscription auth) once.
    /// Permanently `#[ignore]`d — run manually with
    /// `cargo test -p katto real_claude_session_smoke -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "spawns real claude; run manually"]
    async fn real_claude_session_smoke() {
        use crate::sessions::launch::{LaunchSpec, sh_quote, shell_invocation};

        let claude = katto_engine::detect::detect_claude().expect("claude CLI not found");
        let dir = std::env::temp_dir().join("katto-claude-smoke");
        std::fs::create_dir_all(&dir).unwrap();
        let settings = dir.join("smoke.settings.json");
        std::fs::write(&settings, "{}").unwrap();
        let spec = LaunchSpec {
            claude_path: claude,
            cwd: dir.clone(),
            initial_prompt: Some(
                "What is 31217 plus 11202? Reply with just the digits, then wait.".into(),
            ),
            append_system_prompt: None,
            settings_path: settings,
            permission_mode: None,
        };
        let pool = SessionPool::new();
        let mut smoke_task = task("smoke");
        smoke_task.cwd = dir;
        let id = pool
            .spawn_headless(
                smoke_task,
                Program::Custom(format!("zsh -lc {}", sh_quote(&shell_invocation(&spec)))),
            )
            .await
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        let mut seen = false;
        while std::time::Instant::now() < deadline {
            let text = String::from_utf8_lossy(&pool.scrollback(&id).unwrap()).to_string();
            // The sum never appears in the prompt echo, so seeing it proves a
            // real model turn came back through the PTY.
            if text.contains("42419") {
                seen = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        pool.close(&id, CloseReason::UserClosed).await.unwrap();
        assert!(seen, "never saw the model's answer in scrollback");
    }

    #[tokio::test]
    async fn resize_propagates_without_error() {
        let pool = SessionPool::new();
        let id = pool
            .spawn_headless(task("t"), Program::Custom("bash -c 'cat'".into()))
            .await
            .unwrap();
        pool.resize(&id, 200, 50).unwrap();
        pool.close(&id, CloseReason::UserClosed).await.unwrap();
    }
}
