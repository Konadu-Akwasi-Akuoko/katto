use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// One progress tick for a job, streamed to `subscribe_job_progress` channels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
pub struct JobProgress {
    pub job_id: String,
    /// 0.0..=1.0
    pub progress: f64,
    pub message: Option<String>,
}

/// A progress consumer. `send` returns `false` when the receiver is gone so
/// the hub can prune it.
pub trait ProgressSink: Send + 'static {
    fn send(&self, update: &JobProgress) -> bool;
}

/// Fan-out of job progress to any number of subscribed sinks.
#[derive(Default)]
pub struct ProgressHub {
    sinks: Mutex<HashMap<String, Vec<Box<dyn ProgressSink>>>>,
}

impl ProgressHub {
    /// Attach a sink to a job's progress stream.
    pub fn subscribe(&self, job_id: &str, sink: Box<dyn ProgressSink>) {
        let Ok(mut sinks) = self.sinks.lock() else {
            return;
        };
        sinks.entry(job_id.to_owned()).or_default().push(sink);
    }

    /// Deliver `update` to every sink of its job, dropping sinks whose
    /// receiver is gone.
    pub fn publish(&self, update: &JobProgress) {
        let Ok(mut sinks) = self.sinks.lock() else {
            return;
        };
        if let Some(list) = sinks.get_mut(&update.job_id) {
            list.retain(|sink| sink.send(update));
            if list.is_empty() {
                sinks.remove(&update.job_id);
            }
        }
    }

    /// Drop every sink for a job (called on terminal state, after the final
    /// publish).
    pub fn clear(&self, job_id: &str) {
        let Ok(mut sinks) = self.sinks.lock() else {
            return;
        };
        sinks.remove(job_id);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    struct StubSink {
        seen: Arc<Mutex<Vec<JobProgress>>>,
        alive: Arc<AtomicBool>,
    }

    impl ProgressSink for StubSink {
        fn send(&self, update: &JobProgress) -> bool {
            self.seen.lock().unwrap().push(update.clone());
            self.alive.load(Ordering::Relaxed)
        }
    }

    fn stub() -> (Box<StubSink>, Arc<Mutex<Vec<JobProgress>>>, Arc<AtomicBool>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let alive = Arc::new(AtomicBool::new(true));
        let sink = Box::new(StubSink {
            seen: seen.clone(),
            alive: alive.clone(),
        });
        (sink, seen, alive)
    }

    fn tick(job_id: &str, progress: f64) -> JobProgress {
        JobProgress {
            job_id: job_id.to_owned(),
            progress,
            message: None,
        }
    }

    #[test]
    fn publish_reaches_only_matching_job() {
        let hub = ProgressHub::default();
        let (sink_a, seen_a, _) = stub();
        let (sink_b, seen_b, _) = stub();
        hub.subscribe("a", sink_a);
        hub.subscribe("b", sink_b);

        hub.publish(&tick("a", 0.5));

        assert_eq!(seen_a.lock().unwrap().len(), 1);
        assert!(seen_b.lock().unwrap().is_empty());
    }

    #[test]
    fn dead_sink_dropped_after_failed_send() {
        let hub = ProgressHub::default();
        let (sink, seen, alive) = stub();
        hub.subscribe("a", sink);

        alive.store(false, Ordering::Relaxed);
        hub.publish(&tick("a", 0.3)); // delivered, then pruned
        hub.publish(&tick("a", 0.6)); // no receiver left

        assert_eq!(seen.lock().unwrap().len(), 1);
    }

    #[test]
    fn clear_removes_all_sinks_for_job() {
        let hub = ProgressHub::default();
        let (sink, seen, _) = stub();
        hub.subscribe("a", sink);

        hub.clear("a");
        hub.publish(&tick("a", 1.0));

        assert!(seen.lock().unwrap().is_empty());
    }
}
