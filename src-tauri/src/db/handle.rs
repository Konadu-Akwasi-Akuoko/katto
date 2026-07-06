use std::path::PathBuf;

use rusqlite::Connection;
use tokio::sync::{mpsc, oneshot};

use crate::error::{Error, Result};

type DbJob = Box<dyn FnOnce(&mut Connection) + Send>;

/// Async handle to the single dedicated SQLite writer thread.
///
/// The connection lives on one owned thread; every clone of the handle funnels
/// closures to it over a channel, so all database access is serialized through a
/// single writer (folders are truth; this index is never touched concurrently).
/// Cheap to clone.
#[derive(Clone)]
pub struct DbHandle {
    sender: mpsc::UnboundedSender<DbJob>,
}

impl DbHandle {
    /// Spawn the writer thread. It opens and migrates the database at `path`, then
    /// serves closures until the last `DbHandle` clone is dropped. Blocks until the
    /// open+migrate step finishes so a failure surfaces synchronously to the caller.
    ///
    /// # Errors
    /// Fails if the writer thread cannot be spawned or the database cannot be
    /// opened/migrated.
    pub fn spawn(path: PathBuf) -> Result<DbHandle> {
        let (job_tx, mut job_rx) = mpsc::unbounded_channel::<DbJob>();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<Result<()>>(0);

        std::thread::Builder::new()
            .name("katto-db".to_string())
            .spawn(move || {
                let mut conn = match super::open(&path) {
                    Ok(conn) => conn,
                    Err(err) => {
                        let _ = ready_tx.send(Err(err));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(()));
                while let Some(job) = job_rx.blocking_recv() {
                    job(&mut conn);
                }
            })?;

        ready_rx.recv().map_err(|_| Error::db_closed())??;
        Ok(DbHandle { sender: job_tx })
    }

    /// Run `f` on the writer thread and await its result.
    ///
    /// # Errors
    /// Returns [`Error::DbClosed`] if the writer thread is gone, otherwise whatever
    /// `f` returns.
    pub async fn call<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut Connection) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(Box::new(move |conn| {
                let _ = tx.send(f(conn));
            }))
            .map_err(|_| Error::db_closed())?;
        rx.await.map_err(|_| Error::db_closed())?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::settings;

    #[test]
    fn call_round_trips_through_writer() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let db = DbHandle::spawn(dir.path().join("katto.db")).unwrap();

            db.call(|conn| settings::set(conn, "studio_root", "/Volumes/Studio"))
                .await
                .unwrap();
            let got = db
                .call(|conn| settings::get(conn, "studio_root"))
                .await
                .unwrap();

            assert_eq!(got.as_deref(), Some("/Volumes/Studio"));
        });
    }
}
