//! End-to-end: prepared bundle -> mocked ElevenLabs -> stub planner -> validated
//! cuts on disk. No network, no ffmpeg, no claude binary.

use katto_engine::Rational;
use katto_engine::bundle::{self, CUTS_JSON, PROJECT_JSON, TRANSCRIPT_JSON};
use katto_engine::planner::{CutPlanner, PlanError, parse_cuts_json};
use katto_engine::schema::{Cuts, Transcript};
use katto_engine::transcribe::{TranscribeConfig, transcribe_into_bundle};
use katto_engine::validate::validate_cuts;

struct CannedPlanner {
    raw: String,
}

impl CutPlanner for CannedPlanner {
    async fn plan(&self, t: &Transcript) -> Result<Cuts, PlanError> {
        let cuts = parse_cuts_json(&self.raw)?;
        assert!(validate_cuts(&cuts, t).is_empty());
        Ok(cuts)
    }
}

#[tokio::test]
async fn transcribe_then_plan_lands_validated_cuts_in_bundle() {
    // bundle scaffold
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("clip.mp4");
    std::fs::write(&source, b"fake").unwrap();
    let root = dir.path().join("clip.kruproj");
    std::fs::create_dir(&root).unwrap();
    bundle::write_json_atomic(
        &root.join(PROJECT_JSON),
        &katto_engine::schema::ProjectManifest {
            schema_version: 1,
            source_video_absolute_path: source.clone(),
            frame_rate: Rational::new(30000, 1001),
            duration: Rational::new(3_843_840, 30000),
        },
    )
    .unwrap();
    std::fs::write(root.join("cached_audio.wav"), b"RIFF").unwrap();

    // mocked ElevenLabs
    let server = wiremock::MockServer::start().await;
    let transcript_raw = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/transcript.valid.json"
    ))
    .unwrap();
    wiremock::Mock::given(wiremock::matchers::path("/v1/speech-to-text"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(transcript_raw))
        .mount(&server)
        .await;
    let cfg = TranscribeConfig {
        api_key: "k".into(),
        base_url: server.uri(),
    };
    let transcript = transcribe_into_bundle(&cfg, &root).await.unwrap();

    // stub planner (canned valid cuts) + persist
    let raw = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/cuts.valid.json"
    ))
    .unwrap();
    let cuts = CannedPlanner { raw }.plan(&transcript).await.unwrap();
    bundle::write_json_atomic(&root.join(CUTS_JSON), &cuts).unwrap();

    // reopen: full bundle materializes
    let b = bundle::open(&root).unwrap();
    assert!(b.transcript.is_some());
    assert_eq!(b.cuts.unwrap().cuts.len(), 2);
    assert!(root.join(TRANSCRIPT_JSON).exists());
}

/// True end-to-end with real binaries and keys — owner checkpoint. Chains
/// import -> transcribe (ELEVENLABS_API_KEY) -> subprocess claude planner.
#[tokio::test]
#[ignore = "requires ffmpeg + claude + real keys; owner checkpoint"]
async fn full_pipeline_real_binaries() {
    let Ok(clip) = std::env::var("KATTO_TEST_CLIP") else {
        eprintln!("set KATTO_TEST_CLIP=/path/to/clip.mp4 to run");
        return;
    };
    let Ok(key) = std::env::var("ELEVENLABS_API_KEY") else {
        eprintln!("set ELEVENLABS_API_KEY to run");
        return;
    };
    let claude = katto_engine::detect::detect_claude();
    let Some(claude_path) = claude else {
        eprintln!("no claude binary on PATH");
        return;
    };

    let dir = tempfile::tempdir().unwrap();
    let out = katto_engine::import::import(std::path::Path::new(&clip), dir.path())
        .await
        .unwrap();
    let cfg = TranscribeConfig {
        api_key: key,
        base_url: katto_engine::transcribe::ELEVENLABS_BASE_URL.into(),
    };
    let transcript = transcribe_into_bundle(&cfg, &out.bundle_root)
        .await
        .unwrap();
    let planner = katto_engine::planner::subprocess::SubprocessClaudePlanner::new(
        claude_path,
        out.bundle_root.clone(),
    );
    let cuts = planner.plan(&transcript).await.unwrap();
    assert!(validate_cuts(&cuts, &transcript).is_empty());
    bundle::write_json_atomic(&out.bundle_root.join(CUTS_JSON), &cuts).unwrap();
    assert!(bundle::open(&out.bundle_root).is_ok());
}
