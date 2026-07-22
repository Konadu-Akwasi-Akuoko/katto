//! The ingest domain: copy job, offer assembly, source validation, and the
//! per-footage-dir serialization that keeps concurrent imports from planning
//! colliding sequence numbers. Pure recognition/enumeration/naming/verification
//! live in `katto_engine::ingest`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

pub mod copy;
pub mod offer;
pub mod validate;

/// One async mutex per footage directory. A job's guard is acquired before the
/// existing-sequence read and held until the copy job ends, so two imports into
/// the same project serialize instead of planning identical `YYYY-MM-DD_NNN`
/// names. The registry itself never shrinks — a handful of project dirs per
/// session, each entry a few words.
static FOOTAGE_LOCKS: LazyLock<Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Acquire the serialization guard for `footage_dir`, waiting if another
/// import into the same directory is planning or copying.
pub async fn lock_footage_dir(footage_dir: &Path) -> tokio::sync::OwnedMutexGuard<()> {
    let mutex = {
        let mut registry = match FOOTAGE_LOCKS.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        registry
            .entry(footage_dir.to_path_buf())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    };
    mutex.lock_owned().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_footage_dir_serializes_second_import() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.block_on(async {
            let dir = PathBuf::from("/studio/Projects/a/footage");
            let other = PathBuf::from("/studio/Projects/b/footage");
            let guard = lock_footage_dir(&dir).await;

            // Same dir: a second acquisition must wait (try_lock fails).
            let same = {
                let registry = FOOTAGE_LOCKS.lock().unwrap();
                registry.get(&dir).unwrap().clone()
            };
            assert!(same.try_lock().is_err());

            // Different dir: independent.
            let _other_guard = lock_footage_dir(&other).await;
            drop(guard);
            assert!(same.try_lock().is_ok());
        });
    }
}
