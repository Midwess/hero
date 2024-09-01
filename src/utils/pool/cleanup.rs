use std::{marker::PhantomData, sync::Arc, time::Duration};

use tokio::{spawn, sync::RwLock, task::JoinHandle, time::sleep};

use super::allocator::{PoolItem, PoolResource};

pub enum CleanupStrategy {
    Easy,
    Best {interval: Duration, min_pool_size: u16},
}

pub struct PoolCleanup<T, P> where T: PoolResource<P> + 'static, P: Send + Sync + 'static {
    _marker: PhantomData<P>,
    strategy: CleanupStrategy,
    pool: Arc<RwLock<Vec<PoolItem<T, P>>>>,
    background_handler: Option<JoinHandle<()>>
}

impl<T,P> PoolCleanup<T, P> where T: PoolResource<P> + 'static, P: Send + Sync + 'static {
    pub fn new(pool: Arc<RwLock<Vec<PoolItem<T, P>>>>, strategy: CleanupStrategy) -> Self {
        Self {
            _marker: PhantomData::default(),
            strategy,
            pool,
            background_handler: None,
        }
    }

    pub fn start_background(&mut self) {
        match self.strategy {
           CleanupStrategy::Best { interval, min_pool_size } => {
               let interval = interval.clone();
               let pool = self.pool.clone();
               let handler = spawn(async move {
                    loop {
                        sleep(interval).await;
                        Self::request_cleanup_loop(&pool, min_pool_size as usize).await
                    }
               });

               self.background_handler = Some(handler);
           },
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

    pub async fn request_cleanup_loop(pool: &Arc<RwLock<Vec<PoolItem<T, P>>>>, from_index: usize) {
        let mut pool = pool.write().await;
        let mut out_dated_indices = Vec::<usize>::new();
        for (index, pool_item) in pool.iter_mut().enumerate() {
            if index >= from_index {
                if pool_item.invalidate() {
                    out_dated_indices.push(index);
                }
            }
        }

        for out_dated_indice in out_dated_indices {
            loop {
                let item = pool.get(out_dated_indice);
                if item.is_some() && item.unwrap().resource.is_some() {
                    pool.swap_remove(out_dated_indice);
                }
                else {
                    break;
                }
            }
        }
    }
}

impl<T,P> Drop for PoolCleanup<T, P> where T: PoolResource<P> + 'static, P: Send + Sync + 'static {
    fn drop(&mut self) {
        print!("Dropped cleanup");
        self.stop_background();
    }
}

