use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use tokio::spawn;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::sleep;

use super::allocator::PoolItem;

pub enum CleanupStrategy {
    Relax { interval: Duration }
}

pub struct PoolCleaner<T>
where
    T: Send + Sync + 'static
{
    strategy: CleanupStrategy,
    pool: Arc<Mutex<VecDeque<PoolItem<T>>>>,
    background_handler: Option<JoinHandle<()>>
}

impl<T> PoolCleaner<T>
where
    T: Send + Sync + 'static,
{
    pub fn new(pool: Arc<Mutex<VecDeque<PoolItem<T>>>>, strategy: CleanupStrategy) -> Self {
        Self {
            strategy,
            pool,
            background_handler: None
        }
    }

    pub fn start_background(&mut self) {
        match self.strategy {
            CleanupStrategy::Relax {
                interval
            } => {
                let interval = interval.clone();
                let pool = self.pool.clone();
                let handler = spawn(async move {
                    loop {
                        sleep(interval).await;
                        Self::request_cleanup_loop(&pool).await
                    }
                });

                self.background_handler = Some(handler);
            }
            _ => {
                // Not support for other task
            }
        }
    }

    pub fn stop_background(&mut self) {
        if self.background_handler.is_some() {
            self.background_handler.take().unwrap().abort();
        }
    }

    pub async fn request_cleanup_loop(pool_sync: &Arc<Mutex<VecDeque<PoolItem<T>>>>) {
        let mut pool = pool_sync.lock().await;

        pool.retain_mut(|it| !it.invalidate());
    }
}

impl<T> Drop for PoolCleaner<T>
where
    T: Send + Sync + 'static,
{
    fn drop(&mut self) {
        self.stop_background();
    }
}
