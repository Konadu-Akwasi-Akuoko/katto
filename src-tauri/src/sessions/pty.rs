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
