use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::Duration;

use tiny_http::{Response, Server};

use crate::error::{Error, Result};

/// Hook payloads larger than this are truncated before parsing; only
/// `hook_event_name` is read, always near the front of Claude's JSON.
const MAX_BODY_BYTES: usize = 64 * 1024;

/// A hook delivery from a session's generated `--settings` hooks. The session
/// id comes from the URL path katto baked into the curl command.
#[derive(Debug, PartialEq)]
pub enum HookEvent {
    Stop {
        session_id: String,
    },
    Notification {
        session_id: String,
    },
    /// The accept thread is exiting on a server error: hook delivery is dead
    /// app-wide (sessions degrade to the silence heuristic). Dispatched so the
    /// pool can record it — the endpoint itself has no DB access.
    EndpointDied {
        error: String,
    },
}

/// The localhost hook receiver: one synchronous accept thread, mirroring the
/// DB-writer thread pattern. The token authenticates every request; it lives
/// only here and in the per-session settings files (0600).
pub struct HooksEndpoint {
    pub port: u16,
    pub token: String,
    stop: Arc<AtomicBool>,
}

/// Bind `127.0.0.1:0` and start the accept thread; dispatches parsed hook
/// events to `on_event`. A closed receiver just stops dispatch — lifecycle is
/// owned by the session pool.
pub fn start(on_event: Sender<HookEvent>) -> Result<HooksEndpoint> {
    let server = Server::http("127.0.0.1:0")
        .map_err(|err| Error::SessionSpawn(format!("hooks endpoint bind failed: {err}")))?;
    let port = server
        .server_addr()
        .to_ip()
        .map(|addr| addr.port())
        .ok_or_else(|| Error::SessionSpawn("hooks endpoint has no IP addr".to_string()))?;
    let token = uuid::Uuid::new_v4().to_string();
    let stop = Arc::new(AtomicBool::new(false));

    let thread_stop = Arc::clone(&stop);
    let thread_token = token.clone();
    std::thread::spawn(move || {
        while !thread_stop.load(Ordering::Relaxed) {
            match server.recv_timeout(Duration::from_millis(250)) {
                Ok(Some(request)) => handle(request, &thread_token, &on_event),
                Ok(None) => {}
                Err(err) => {
                    let _ = on_event.send(HookEvent::EndpointDied {
                        error: err.to_string(),
                    });
                    break;
                }
            }
        }
    });

    Ok(HooksEndpoint { port, token, stop })
}

fn handle(mut request: tiny_http::Request, token: &str, on_event: &Sender<HookEvent>) {
    let status = respond_status(&mut request, token, on_event);
    let _ = request.respond(Response::empty(status));
}

fn respond_status(
    request: &mut tiny_http::Request,
    token: &str,
    on_event: &Sender<HookEvent>,
) -> u16 {
    if request.method() != &tiny_http::Method::Post {
        return 404;
    }
    let session_id = match request.url().strip_prefix("/hook/") {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => return 404,
    };
    let authed = request
        .headers()
        .iter()
        .any(|h| h.field.equiv("x-katto-token") && token_matches(h.value.as_str(), token));
    if !authed {
        return 401;
    }
    let mut body = Vec::new();
    let _ = request
        .as_reader()
        .take(MAX_BODY_BYTES as u64)
        .read_to_end(&mut body);
    let event_name = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v["hook_event_name"].as_str().map(str::to_string));
    let event = match event_name.as_deref() {
        Some("Stop") => HookEvent::Stop { session_id },
        Some("Notification") => HookEvent::Notification { session_id },
        _ => return 200,
    };
    let _ = on_event.send(event);
    200
}

impl HooksEndpoint {
    /// Signal the accept thread to exit after its current 250 ms poll.
    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Constant-time token comparison: an early-exit `==` would let a local
/// process narrow the token byte-by-byte through response timing. Length is
/// not secret (it is a fixed-width UUID).
fn token_matches(candidate: &str, token: &str) -> bool {
    let a = candidate.as_bytes();
    let b = token.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    fn post(port: u16, path: &str, token: Option<&str>, body: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let token_header = token
            .map(|t| format!("x-katto-token: {t}\r\n"))
            .unwrap_or_default();
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
        let resp = post(
            ep.port,
            "/hook/sess-9",
            Some(&ep.token),
            r#"{"hook_event_name":"Stop"}"#,
        );
        assert!(resp.starts_with("HTTP/1.1 200"));
        let event = rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
        assert_eq!(
            event,
            HookEvent::Stop {
                session_id: "sess-9".into()
            }
        );
        ep.shutdown();
    }

    #[test]
    fn bad_token_is_rejected_without_event() {
        let (tx, rx) = std::sync::mpsc::channel();
        let ep = start(tx).unwrap();
        let resp = post(
            ep.port,
            "/hook/sess-9",
            Some("wrong"),
            r#"{"hook_event_name":"Stop"}"#,
        );
        assert!(resp.starts_with("HTTP/1.1 401"));
        assert!(
            rx.recv_timeout(std::time::Duration::from_millis(300))
                .is_err()
        );
        ep.shutdown();
    }

    #[test]
    fn notification_hook_maps_to_needs_input_event() {
        let (tx, rx) = std::sync::mpsc::channel();
        let ep = start(tx).unwrap();
        post(
            ep.port,
            "/hook/s1",
            Some(&ep.token),
            r#"{"hook_event_name":"Notification","message":"perm"}"#,
        );
        assert_eq!(
            rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap(),
            HookEvent::Notification {
                session_id: "s1".into()
            }
        );
        ep.shutdown();
    }

    #[test]
    fn token_matches_only_on_exact_equality() {
        assert!(token_matches("abc-123", "abc-123"));
        assert!(!token_matches("abc-124", "abc-123"));
        assert!(!token_matches("abc-12", "abc-123"));
        assert!(!token_matches("", "abc-123"));
    }

    #[test]
    fn unknown_path_is_404() {
        let (tx, rx) = std::sync::mpsc::channel();
        let ep = start(tx).unwrap();
        let resp = post(ep.port, "/nope", Some(&ep.token), "{}");
        assert!(resp.starts_with("HTTP/1.1 404"));
        assert!(
            rx.recv_timeout(std::time::Duration::from_millis(300))
                .is_err()
        );
        ep.shutdown();
    }
}
