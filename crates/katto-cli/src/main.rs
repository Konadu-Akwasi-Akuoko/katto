//! Thin entry: parse args, dispatch to engine calls, map errors to exit 1.

mod cli;
mod keys;
mod output;

use std::path::{Path, PathBuf};

use clap::Parser;
use katto_engine::planner::subprocess::SubprocessClaudePlanner;
use katto_engine::planner::{CutPlanner, Planner, http::HttpAnthropicPlanner};
use katto_engine::schema::{Cuts, Transcript};
use katto_engine::transcribe::{ELEVENLABS_BASE_URL, TranscribeConfig};

use crate::cli::{AuthCmd, Cli, Command, PlannerChoice};
use crate::keys::{KeyName, resolve};
use crate::output::AuthStatus;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    keys::init_keychain();
    if let Err(e) = run(cli).await {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    let json = cli.json;
    match cli.command {
        Command::Import { video, project } => {
            let outcome = import(&video, project.as_deref()).await?;
            println!("{}", output::render_import(&outcome, json));
        }
        Command::Transcribe { bundle } => {
            let transcript = transcribe(&bundle).await?;
            println!(
                "{}",
                output::render_transcribe(&bundle, transcript.words.len(), json)
            );
        }
        Command::Plan {
            bundle,
            planner,
            model,
        } => {
            let cuts = plan(&bundle, planner, model).await?;
            println!("{}", output::render_plan(&bundle, &cuts, json));
        }
        Command::Cut {
            video,
            project,
            planner,
        } => {
            let outcome = import(&video, project.as_deref()).await?;
            println!("{}", output::render_import(&outcome, json));
            let transcript = transcribe(&outcome.bundle_root).await?;
            println!(
                "{}",
                output::render_transcribe(&outcome.bundle_root, transcript.words.len(), json)
            );
            let cuts = plan(&outcome.bundle_root, planner, None).await?;
            println!("{}", output::render_plan(&outcome.bundle_root, &cuts, json));
        }
        Command::Auth {
            cmd: AuthCmd::Status,
        } => {
            let status = AuthStatus {
                claude_path: katto_engine::detect::detect_claude().map(|p| p.display().to_string()),
                elevenlabs: resolve(KeyName::Elevenlabs).1,
                anthropic: resolve(KeyName::Anthropic).1,
            };
            println!("{}", output::render_auth_status(&status, json));
        }
    }
    Ok(())
}

/// Import into `<project>/audio/` when given, else beside the video (loose bundle).
async fn import(
    video: &Path,
    project: Option<&Path>,
) -> anyhow::Result<katto_engine::import::ImportOutcome> {
    let parent = match project {
        Some(dir) => {
            let audio = dir.join("audio");
            std::fs::create_dir_all(&audio)?;
            audio
        }
        None => video
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| anyhow::anyhow!("video path has no parent directory"))?,
    };
    Ok(katto_engine::import::import(video, &parent).await?)
}

async fn transcribe(bundle: &Path) -> anyhow::Result<Transcript> {
    let Some(api_key) = resolve(KeyName::Elevenlabs).0 else {
        anyhow::bail!(
            "no ElevenLabs API key: set ELEVENLABS_API_KEY or store one in the katto keychain"
        );
    };
    let cfg = TranscribeConfig {
        api_key,
        base_url: ELEVENLABS_BASE_URL.into(),
    };
    Ok(katto_engine::transcribe::transcribe_into_bundle(&cfg, bundle).await?)
}

async fn plan(
    bundle: &Path,
    choice: Option<PlannerChoice>,
    model: Option<String>,
) -> anyhow::Result<Cuts> {
    let opened = katto_engine::bundle::open_unchecked(bundle)?;
    let Some(transcript) = opened.transcript else {
        anyhow::bail!("bundle has no transcript yet - run `katto transcribe` first");
    };
    let planner = select_planner(choice, model, bundle)?;
    let cuts = planner
        .plan(&transcript)
        .await
        .map_err(katto_engine::Error::from)?;
    katto_engine::bundle::write_json_atomic(&bundle.join(katto_engine::bundle::CUTS_JSON), &cuts)?;
    Ok(cuts)
}

fn select_planner(
    choice: Option<PlannerChoice>,
    model: Option<String>,
    workdir: &Path,
) -> anyhow::Result<Planner> {
    let subprocess = |path: PathBuf| {
        Planner::Subprocess(SubprocessClaudePlanner::new(path, workdir.to_path_buf()))
    };
    let http = |key: String| {
        Planner::Http(HttpAnthropicPlanner {
            api_key: key,
            model: model
                .clone()
                .unwrap_or_else(|| katto_engine::planner::http::DEFAULT_MODEL.into()),
            base_url: katto_engine::planner::http::ANTHROPIC_BASE_URL.into(),
        })
    };
    match choice {
        Some(PlannerChoice::Subprocess) => katto_engine::detect::detect_claude()
            .map(subprocess)
            .ok_or_else(|| anyhow::anyhow!("--planner subprocess but no claude binary found")),
        Some(PlannerChoice::Http) => resolve(KeyName::Anthropic)
            .0
            .map(http)
            .ok_or_else(|| anyhow::anyhow!("--planner http but no Anthropic API key found")),
        None => {
            if let Some(path) = katto_engine::detect::detect_claude() {
                return Ok(subprocess(path));
            }
            if let Some(key) = resolve(KeyName::Anthropic).0 {
                return Ok(http(key));
            }
            anyhow::bail!(
                "no planner available: install claude or store an Anthropic API key (katto auth status)"
            )
        }
    }
}
