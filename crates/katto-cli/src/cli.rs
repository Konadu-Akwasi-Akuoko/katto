//! Clap surface for the `katto` binary. Phase 5 appends `render`/`export`
//! variants to [`Command`]; no restructuring needed.

use std::path::PathBuf;

#[derive(clap::Parser)]
#[command(name = "katto", version, about = "katto cut pipeline CLI")]
pub struct Cli {
    /// Emit machine-readable JSON instead of human output.
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(clap::Subcommand)]
pub enum Command {
    /// Probe a clip and create its .kruproj bundle with extracted audio.
    Import {
        /// The source video file.
        video: PathBuf,
        /// Project directory; the bundle lands in its audio/ subdirectory.
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Transcribe a bundle's cached audio with ElevenLabs Scribe v2.
    Transcribe {
        /// The .kruproj bundle directory.
        bundle: PathBuf,
    },
    /// Plan cuts for a transcribed bundle.
    Plan {
        /// The .kruproj bundle directory.
        bundle: PathBuf,
        /// Force a planner instead of auto-detection.
        #[arg(long)]
        planner: Option<PlannerChoice>,
        /// Model id for the HTTP planner (default claude-sonnet-4-6).
        #[arg(long)]
        model: Option<String>,
    },
    /// import + transcribe + plan in one shot.
    Cut {
        /// The source video file.
        video: PathBuf,
        /// Project directory; the bundle lands in its audio/ subdirectory.
        #[arg(long)]
        project: Option<PathBuf>,
        /// Force a planner instead of auto-detection.
        #[arg(long)]
        planner: Option<PlannerChoice>,
    },
    /// Show claude detection and key presence.
    Auth {
        #[command(subcommand)]
        cmd: AuthCmd,
    },
    /// Render the kept-only MP4 for a bundle.
    Render {
        /// The .kruproj bundle directory.
        bundle: PathBuf,
        /// Output file (default: the project's exports/ with a -vN name).
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Export FCPXML + SRT/VTT into the project's timelines/ directory.
    Export {
        /// The .kruproj bundle directory.
        bundle: PathBuf,
    },
}

/// Where a bundle's exports belong (engine-shared logic; the app crate uses
/// the same resolution).
pub fn project_context(bundle_root: &std::path::Path) -> (PathBuf, String) {
    katto_engine::timelines::project_context(bundle_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_context_detects_audio_parent() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("proj").join("audio").join("c.kruproj");
        std::fs::create_dir_all(&root).unwrap();
        let (timelines, slug) = project_context(&root);
        assert_eq!(timelines, dir.path().join("proj").join("timelines"));
        assert_eq!(slug, "proj");
    }

    #[test]
    fn project_context_loose_bundle_falls_back_to_sibling() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("c.kruproj");
        std::fs::create_dir_all(&root).unwrap();
        let (timelines, slug) = project_context(&root);
        assert_eq!(timelines, dir.path().join("timelines"));
        assert_eq!(slug, "c");
    }
}

#[derive(clap::Subcommand)]
pub enum AuthCmd {
    /// Report claude detection and key sources without exposing values.
    Status,
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum PlannerChoice {
    /// Subscription-auth claude subprocess.
    Subprocess,
    /// BYOK Anthropic Messages API.
    Http,
}
