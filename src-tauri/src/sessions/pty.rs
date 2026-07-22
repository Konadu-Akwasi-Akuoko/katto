use std::io::{Read, Write};
use std::path::Path;

use portable_pty::{Child, ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::error::{Error, Result};

/// Everything the pool needs from one spawned PTY. The slave half is dropped
/// before returning so EOF propagates when the child exits.
pub struct PtyHandle {
    pub writer: Box<dyn Write + Send>,
    pub master: Box<dyn MasterPty + Send>,
    pub killer: Box<dyn ChildKiller + Send + Sync>,
    pub reader: Box<dyn Read + Send>,
    pub child: Box<dyn Child + Send + Sync>,
}

/// The ONLY portable-pty call site. Thin by design (testing rules): covered by
/// the pool integration tests against fake shells, not unit-tested itself.
pub fn spawn_pty(
    program: &str,
    args: &[&str],
    cwd: &Path,
    cols: u16,
    rows: u16,
) -> Result<PtyHandle> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|err| Error::SessionSpawn(format!("openpty failed: {err}")))?;

    let mut cmd = CommandBuilder::new(program);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.cwd(cwd);
    cmd.env("TERM", "xterm-256color");
    scrub_env(&mut cmd);

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|err| Error::SessionSpawn(format!("pty spawn failed: {err}")))?;
    drop(pair.slave);

    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|err| Error::SessionSpawn(format!("pty reader failed: {err}")))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|err| Error::SessionSpawn(format!("pty writer failed: {err}")))?;
    let killer = child.clone_killer();

    Ok(PtyHandle {
        writer,
        master: pair.master,
        killer,
        reader,
        child,
    })
}

/// Drop Anthropic auth/routing vars from a session's env. An exported
/// `ANTHROPIC_API_KEY` would silently flip every claude session from
/// subscription auth to per-token API billing (non-negotiable invariant);
/// `ANTHROPIC_BASE_URL` could redirect traffic entirely. Belt half of the
/// belt-and-braces scrub — [`super::launch::shell_invocation`] re-unsets the
/// same vars after `zsh -l` profile sourcing.
fn scrub_env(cmd: &mut CommandBuilder) {
    for var in crate::sessions::launch::SCRUBBED_ENV_VARS {
        cmd.env_remove(var);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::launch::SCRUBBED_ENV_VARS;

    #[test]
    fn scrub_env_removes_anthropic_vars_present_in_parent_env() {
        let mut cmd = CommandBuilder::new("true");
        for var in SCRUBBED_ENV_VARS {
            cmd.env(var, "leaked-value");
        }
        cmd.env("TERM", "xterm-256color");

        scrub_env(&mut cmd);

        for var in SCRUBBED_ENV_VARS {
            assert!(
                cmd.get_env(var).is_none(),
                "{var} must not reach the session env"
            );
        }
        assert!(cmd.get_env("TERM").is_some(), "unrelated env survives");
    }
}
