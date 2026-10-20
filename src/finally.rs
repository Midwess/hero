//! Best-effort asynchronous cleanup tied to an owner's lifetime.
//!
//! Clones contribute cleanup futures to the same collection. Dropping any
//! clone schedules the collection and takes all currently registered work;
//! later registrations form a new collection for a later drop. Cleanup runs
//! sequentially in registration order on the runtime used by `n0-future`.

use futures_util::lock::Mutex;
use n0_future::task::spawn;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

type Cleanup = Pin<Box<dyn Future<Output = ()> + Send>>;

#[derive(Default, Clone)]
/// A shared collection of cleanup futures scheduled when an owner is dropped.
pub struct Finally {
    cleanup_tasks: Arc<Mutex<Option<Vec<Cleanup>>>>,
}

impl Finally {
    /// Adds cleanup work to the next drop-triggered cleanup batch.
    ///
    /// Register cleanup before the owning value begins shutting down. A task
    /// added after another clone has already taken the batch waits for a later
    /// clone to be dropped.
    pub async fn add_cleanup<T>(&self, task: T)
    where
        T: Future<Output = ()> + Send + 'static,
    {
        let mut tasks = self.cleanup_tasks.lock().await;
        if tasks.is_none() {
            *tasks = Some(vec![]);
        }

        if let Some(tasks) = tasks.as_mut() {
            tasks.push(Box::pin(task));
        }
    }
}

impl Drop for Finally {
    fn drop(&mut self) {
        let tasks_arc = self.cleanup_tasks.clone();
        spawn(async move {
            let tasks = tasks_arc.lock().await.take();
            if let Some(tasks) = tasks {
                for task in tasks {
                    task.await;
                }
            }
        });
    }
}
