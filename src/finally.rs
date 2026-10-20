use futures_util::lock::Mutex;
use n0_future::task::spawn;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

type Cleanup = Pin<Box<dyn Future<Output = ()> + Send>>;

#[derive(Default, Clone)]
pub struct Finally {
    cleanup_tasks: Arc<Mutex<Option<Vec<Cleanup>>>>,
}

impl Finally {
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
