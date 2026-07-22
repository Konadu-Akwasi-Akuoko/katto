//! Exit-code contract: 0 ok, 1 pipeline error, 2 usage (clap default).

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;

#[test]
fn no_args_is_usage_error_exit_2() {
    Command::cargo_bin("katto").unwrap().assert().code(2);
}

#[test]
fn import_missing_file_is_pipeline_error_exit_1() {
    Command::cargo_bin("katto")
        .unwrap()
        .args(["import", "/nonexistent/clip.mp4"])
        .assert()
        .code(1)
        .stderr(predicates::str::is_empty().not());
}

#[test]
fn auth_status_exits_0() {
    // Env keys short-circuit resolution so the test never touches the real
    // keychain (a keychain ACL prompt would hang a headless run).
    Command::cargo_bin("katto")
        .unwrap()
        .env("ELEVENLABS_API_KEY", "test-key")
        .env("ANTHROPIC_API_KEY", "test-key")
        .args(["auth", "status", "--json"])
        .assert()
        .code(0);
}

#[test]
fn plan_on_bundle_without_transcript_exits_1() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("katto")
        .unwrap()
        .args(["plan", dir.path().to_str().unwrap()])
        .assert()
        .code(1)
        .stderr(predicates::str::is_empty().not());
}
