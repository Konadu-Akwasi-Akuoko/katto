//! ElevenLabs Scribe v2 client. Keys arrive as arguments (keychain stays in
//! app/CLI), are never logged, and the raw response body is persisted verbatim
//! as `transcript.json` only after a successful parse.

use std::path::Path;
use std::time::Duration;

use crate::bundle::{CACHED_AUDIO_WAV, TRANSCRIPT_JSON, write_atomic};
use crate::error::{Error, Result};
use crate::schema::Transcript;

/// Production ElevenLabs API origin.
pub const ELEVENLABS_BASE_URL: &str = "https://api.elevenlabs.io";

/// Delay before the single retry on 429/5xx (short-circuited in tests).
const RETRY_DELAY: Duration = if cfg!(test) {
    Duration::from_millis(10)
} else {
    Duration::from_secs(5)
};

/// Scribe's direct-upload cap (docs: "must be less than 5.0GB").
const MAX_UPLOAD_BYTES: u64 = 5_000_000_000;

/// TCP connect cap; a black-holed host must fail fast, not hang the job.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Overall per-attempt cap: fixed base for server-side transcription work plus
/// upload headroom at a 1 MB/s floor. Generous by design — jobs have no cancel
/// yet, so the only wrong value is "unbounded".
fn overall_timeout(upload_bytes: u64) -> Duration {
    Duration::from_secs(300 + upload_bytes / 1_000_000)
}

/// Reject uploads Scribe would refuse, before any bytes leave the machine.
fn check_upload_size(bytes: u64) -> Result<()> {
    if bytes > MAX_UPLOAD_BYTES {
        return Err(Error::Transcribe(format!(
            "cached audio is {bytes} bytes; Scribe's direct-upload limit is 5.0 GB"
        )));
    }
    Ok(())
}

/// Scribe v2 request configuration; the key never appears in Debug output.
#[derive(Clone)]
pub struct TranscribeConfig {
    /// The ElevenLabs API key (never logged, never persisted here).
    pub api_key: String,
    /// API origin; injectable for tests.
    pub base_url: String,
}

impl std::fmt::Debug for TranscribeConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TranscribeConfig")
            .field("api_key", &"<redacted>")
            .field("base_url", &self.base_url)
            .finish()
    }
}

/// POST `wav_path` to Scribe v2; returns the raw response body plus the parsed
/// transcript. One automatic retry on 429/5xx after [`RETRY_DELAY`].
///
/// # Errors
/// [`Error::TranscribeAuth`] on 401, [`Error::TranscribeQuota`] on 429 after
/// retry, [`Error::Transcribe`] for every other transport/response failure.
pub async fn transcribe(cfg: &TranscribeConfig, wav_path: &Path) -> Result<(Vec<u8>, Transcript)> {
    let len = tokio::fs::metadata(wav_path).await?.len();
    check_upload_size(len)?;
    match attempt(cfg, wav_path, len).await {
        Err(Retry::After(_)) => {}
        Err(Retry::Fatal(e)) => return Err(e),
        Ok(ok) => return Ok(ok),
    }
    tokio::time::sleep(RETRY_DELAY).await;
    match attempt(cfg, wav_path, len).await {
        Ok(ok) => Ok(ok),
        Err(Retry::After(e) | Retry::Fatal(e)) => Err(e),
    }
}

/// Whether a failed attempt is retryable.
enum Retry {
    /// Retry once after the delay (429/5xx).
    After(Error),
    /// Do not retry (auth, validation, parse, transport).
    Fatal(Error),
}

async fn attempt(
    cfg: &TranscribeConfig,
    wav_path: &Path,
    len: u64,
) -> std::result::Result<(Vec<u8>, Transcript), Retry> {
    // Re-opened per attempt: a streamed body is consumed by the send, so the
    // retry needs a fresh handle. Streaming keeps peak memory flat instead of
    // the whole WAV (plus a clone) resident.
    let file = tokio::fs::File::open(wav_path)
        .await
        .map_err(|e| Retry::Fatal(Error::Io(e.to_string())))?;
    let part = reqwest::multipart::Part::stream_with_length(reqwest::Body::from(file), len)
        .file_name("cached_audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| Retry::Fatal(Error::Transcribe(e.to_string())))?;
    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model_id", "scribe_v2")
        .text("timestamps_granularity", "word")
        .text("diarize", "true")
        .text("tag_audio_events", "true");

    let timeout = if cfg!(test) {
        Duration::from_secs(2)
    } else {
        overall_timeout(len)
    };
    let client = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(timeout)
        .build()
        .map_err(|e| Retry::Fatal(Error::Transcribe(e.to_string())))?;
    let response = client
        .post(format!("{}/v1/speech-to-text", cfg.base_url))
        .header("xi-api-key", &cfg.api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| Retry::Fatal(Error::Transcribe(e.to_string())))?;

    let status = response.status();
    let body = response
        .bytes()
        .await
        .map_err(|e| Retry::Fatal(Error::Transcribe(e.to_string())))?
        .to_vec();
    let body_text = || String::from_utf8_lossy(&body).into_owned();

    match status.as_u16() {
        200 => parse_success(body).map_err(Retry::Fatal),
        // Status only: a 401 body can echo request details, and auth errors
        // reach UI copy — never forward it.
        401 => Err(Retry::Fatal(Error::TranscribeAuth(
            "ElevenLabs rejected the API key (401)".into(),
        ))),
        429 => Err(Retry::After(Error::TranscribeQuota(body_text()))),
        500..=599 => Err(Retry::After(Error::Transcribe(format!(
            "{status}: {}",
            body_text()
        )))),
        _ => Err(Retry::Fatal(Error::Transcribe(format!(
            "{status}: {}",
            body_text()
        )))),
    }
}

fn parse_success(body: Vec<u8>) -> Result<(Vec<u8>, Transcript)> {
    let value: serde_json::Value =
        serde_json::from_slice(&body).map_err(|e| Error::Transcribe(e.to_string()))?;
    if value.get("transcripts").is_some() || value.get("request_id").is_some() {
        return Err(Error::Transcribe(
            "unexpected multichannel/webhook response".into(),
        ));
    }
    let transcript: Transcript =
        serde_json::from_value(value).map_err(|e| Error::Transcribe(e.to_string()))?;
    if transcript.audio_duration_secs.is_none() {
        return Err(Error::Transcribe("missing audio_duration_secs".into()));
    }
    Ok((body, transcript))
}

/// Run [`transcribe`] on the bundle's cached audio and atomically persist the
/// raw body as `transcript.json`. A valid existing `transcript.json` is reused
/// verbatim — a later-stage failure (planner retry, quota) must never re-bill
/// the transcription; a corrupt one is re-transcribed and overwritten.
///
/// # Errors
/// As [`transcribe`], plus [`Error::Io`] on the artifact write.
pub async fn transcribe_into_bundle(
    cfg: &TranscribeConfig,
    bundle_root: &Path,
) -> Result<Transcript> {
    let transcript_path = bundle_root.join(TRANSCRIPT_JSON);
    if let Ok(bytes) = tokio::fs::read(&transcript_path).await
        && let Ok(existing) = serde_json::from_slice::<Transcript>(&bytes)
        && existing.audio_duration_secs.is_some()
    {
        return Ok(existing);
    }
    let (raw, transcript) = transcribe(cfg, &bundle_root.join(CACHED_AUDIO_WAV)).await?;
    write_atomic(&transcript_path, &raw)?;
    Ok(transcript)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

    fn transcript_body() -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/transcript.valid.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    fn wav_file(dir: &tempfile::TempDir) -> std::path::PathBuf {
        let p = dir.path().join("cached_audio.wav");
        std::fs::write(&p, b"RIFFfakewav").unwrap();
        p
    }

    #[tokio::test]
    async fn posts_multipart_and_parses_transcript() {
        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .and(matchers::path("/v1/speech-to-text"))
            .and(matchers::header("xi-api-key", "k123"))
            .respond_with(ResponseTemplate::new(200).set_body_string(transcript_body()))
            .expect(1)
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let cfg = TranscribeConfig {
            api_key: "k123".into(),
            base_url: server.uri(),
        };
        let (raw, t) = transcribe(&cfg, &wav_file(&dir)).await.unwrap();
        assert!(!raw.is_empty());
        assert!(t.audio_duration_secs.is_some());
    }

    #[tokio::test]
    async fn auth_failure_is_typed() {
        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(ResponseTemplate::new(401).set_body_string("bad key"))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let cfg = TranscribeConfig {
            api_key: "bad".into(),
            base_url: server.uri(),
        };
        assert!(matches!(
            transcribe(&cfg, &wav_file(&dir)).await,
            Err(Error::TranscribeAuth(_))
        ));
    }

    #[tokio::test]
    async fn quota_retries_once_then_types() {
        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(ResponseTemplate::new(429).set_body_string("slow down"))
            .expect(2) // initial + one retry
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let cfg = TranscribeConfig {
            api_key: "k".into(),
            base_url: server.uri(),
        };
        assert!(matches!(
            transcribe(&cfg, &wav_file(&dir)).await,
            Err(Error::TranscribeQuota(_))
        ));
    }

    #[tokio::test]
    async fn multichannel_response_is_a_hard_error() {
        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"transcripts":[{}]}"#))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let cfg = TranscribeConfig {
            api_key: "k".into(),
            base_url: server.uri(),
        };
        match transcribe(&cfg, &wav_file(&dir)).await {
            Err(Error::Transcribe(msg)) => assert!(msg.contains("multichannel")),
            other => panic!("expected Transcribe, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn into_bundle_writes_raw_body_atomically() {
        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string(transcript_body()))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("b.kruproj");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("cached_audio.wav"), b"RIFF").unwrap();
        let cfg = TranscribeConfig {
            api_key: "k".into(),
            base_url: server.uri(),
        };
        transcribe_into_bundle(&cfg, &root).await.unwrap();
        let on_disk = std::fs::read_to_string(root.join("transcript.json")).unwrap();
        assert_eq!(on_disk, transcript_body()); // raw body verbatim
        assert!(!root.join("transcript.json.tmp").exists());
    }

    #[test]
    fn debug_redacts_the_key() {
        let cfg = TranscribeConfig {
            api_key: "sk-secret".into(),
            base_url: "http://x".into(),
        };
        let dbg = format!("{cfg:?}");
        assert!(!dbg.contains("sk-secret"));
        assert!(dbg.contains("<redacted>"));
    }

    #[tokio::test]
    async fn into_bundle_reuses_existing_valid_transcript_without_calling_api() {
        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string(transcript_body()))
            .expect(0) // a cached transcript must never re-bill ElevenLabs
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("b.kruproj");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("cached_audio.wav"), b"RIFF").unwrap();
        std::fs::write(root.join("transcript.json"), transcript_body()).unwrap();
        let cfg = TranscribeConfig {
            api_key: "k".into(),
            base_url: server.uri(),
        };
        let t = transcribe_into_bundle(&cfg, &root).await.unwrap();
        assert!(t.audio_duration_secs.is_some());
    }

    #[tokio::test]
    async fn into_bundle_retranscribes_over_a_corrupt_transcript() {
        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string(transcript_body()))
            .expect(1)
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("b.kruproj");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("cached_audio.wav"), b"RIFF").unwrap();
        std::fs::write(root.join("transcript.json"), b"{ not json").unwrap();
        let cfg = TranscribeConfig {
            api_key: "k".into(),
            base_url: server.uri(),
        };
        transcribe_into_bundle(&cfg, &root).await.unwrap();
        let on_disk = std::fs::read_to_string(root.join("transcript.json")).unwrap();
        assert_eq!(on_disk, transcript_body());
    }

    #[test]
    fn upload_size_precheck_enforces_scribe_cap() {
        assert!(check_upload_size(MAX_UPLOAD_BYTES).is_ok());
        let err = check_upload_size(MAX_UPLOAD_BYTES + 1).unwrap_err();
        assert!(matches!(err, Error::Transcribe(_)));
        assert!(err.to_string().contains("5.0 GB"));
    }

    #[test]
    fn overall_timeout_scales_with_upload_size() {
        // 1s of headroom per uploaded MB on top of the fixed base.
        let base = overall_timeout(0);
        assert_eq!(
            overall_timeout(600 * 1_000_000) - base,
            Duration::from_secs(600)
        );
    }

    #[tokio::test]
    async fn black_holed_connection_times_out() {
        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(transcript_body())
                    .set_delay(Duration::from_secs(30)),
            )
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let cfg = TranscribeConfig {
            api_key: "k".into(),
            base_url: server.uri(),
        };
        let started = std::time::Instant::now();
        assert!(matches!(
            transcribe(&cfg, &wav_file(&dir)).await,
            Err(Error::Transcribe(_))
        ));
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[tokio::test]
    async fn auth_error_never_echoes_the_response_body() {
        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .respond_with(ResponseTemplate::new(401).set_body_string("key sk-echoed-back rejected"))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let cfg = TranscribeConfig {
            api_key: "bad".into(),
            base_url: server.uri(),
        };
        let err = transcribe(&cfg, &wav_file(&dir)).await.unwrap_err();
        assert!(!err.to_string().contains("sk-echoed-back"));
    }
}
