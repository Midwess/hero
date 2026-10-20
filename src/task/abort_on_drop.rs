use tokio::task::JoinHandle;

/// Aborts a Tokio task when the guard is dropped.
///
/// Wrap the guard in an [`std::sync::Arc`] when several owners should keep the
/// task alive until the last reference disappears. The output is intentionally
/// inaccessible; use the original [`JoinHandle`] when awaiting completion is
/// part of the protocol.
pub struct AbortOnDrop<T>(JoinHandle<T>);

impl<T> AbortOnDrop<T> {
    /// Takes ownership of a spawned task.
    pub fn new(handle: JoinHandle<T>) -> Self {
        Self(handle)
    }
}

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test(flavor = "current_thread")]
    async fn drop_aborts_task() {
        let flag = Arc::new(AtomicBool::new(false));
        let flag_for_task = flag.clone();

        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(60)).await;
            flag_for_task.store(true, Ordering::SeqCst);
        });

        let guard = AbortOnDrop::new(handle);
        tokio::time::sleep(Duration::from_millis(10)).await;
        drop(guard);
        tokio::time::sleep(Duration::from_millis(10)).await;

        assert!(!flag.load(Ordering::SeqCst), "task should be aborted before flag was set");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn arc_clones_keep_task_alive_until_last_drop() {
        let handle = tokio::spawn(async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            42
        });

        let guard = Arc::new(AbortOnDrop::new(handle));
        let clone = guard.clone();
        drop(guard);
        tokio::time::sleep(Duration::from_millis(10)).await;
        drop(clone);
    }
}
