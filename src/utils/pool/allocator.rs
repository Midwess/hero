use log::*;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{oneshot, Mutex};

use super::cleanup::{CleanupStrategy, PoolCleaner};

#[async_trait::async_trait]
pub trait PoolResourceProvider<T>: Send + Sync
    where T: Send + Sync
{
    async fn new(&self) -> T where Self: 'async_trait;
}

pub struct PoolAllocator<T>
where
    T: 'static + Send + Sync
{
    pool: Arc<Mutex<VecDeque<PoolItem<T>>>>,
    min_pool_size: usize,
    max_pool_size: usize,
    resource_idle_timeout: Duration,
    resource_provider: Box<dyn PoolResourceProvider<T> + 'static>,
    pool_cleaner: PoolCleaner<T>,
    waiters: Mutex<VecDeque<oneshot::Sender<Box<T>>>>,

    current_pool_size: Mutex<usize>,
}

pub struct PoolBuilder<T> where T: Send + Sync + 'static
{
    min_pool_size: usize,
    max_pool_size: usize,
    resource_idle_timeout: Duration,
    pool_resource_provider: Option<Box<dyn PoolResourceProvider<T> + 'static>>,
    cleanup_strategy: Option<CleanupStrategy>
}

impl<T> PoolBuilder<T> where T: Send + Sync + 'static
{
    pub fn new(resource_provider: Box<dyn PoolResourceProvider<T> + 'static>) -> Self {
        Self {
            min_pool_size: 10,
            max_pool_size: 20,
            resource_idle_timeout: Duration::new(10, 0),
            pool_resource_provider: Some(resource_provider),
            cleanup_strategy: Some(CleanupStrategy::Relax {
                interval: Duration::new(10, 0),
            }),
        }
    }

    pub fn resource_idle_timeout(mut self, timeout: Duration) -> Self {
        self.resource_idle_timeout = timeout;

        self
    }

    pub fn min_pool_size(mut self, size: usize) -> Self {
        if size <= 0 {
            panic!("The min pool size must be positive");
        }

        self.min_pool_size = size;

        self
    }

    pub fn max_pool_size(mut self, size: usize) -> Self {
        if size <= 0 {
            panic!("The max pool size must be positive");
        }

        self.max_pool_size = size;
        self
    }

    pub fn cleanup(mut self, strategy: CleanupStrategy) -> Self {
        self.cleanup_strategy = Some(strategy);
        self
    }

    pub async fn build(mut self) -> PoolAllocator<T>
    {
        if self.max_pool_size < self.min_pool_size {
            panic!("The max pool size can not less than min pool size");
        }

        let pool = Arc::new(Mutex::new(VecDeque::with_capacity(self.min_pool_size)));

        let cleaner = PoolCleaner::new(pool.clone(), self.cleanup_strategy.take().unwrap());
        let mut pool_allocator = PoolAllocator {
            max_pool_size: self.max_pool_size,
            min_pool_size: self.min_pool_size,
            resource_provider: self.pool_resource_provider.take().expect("The resource_provider is required"),
            resource_idle_timeout: self.resource_idle_timeout,
            pool,
            current_pool_size: Mutex::new(0),
            waiters: Mutex::new(VecDeque::new()),
            pool_cleaner: cleaner
        };

        pool_allocator.init().await;

        pool_allocator
    }

    pub async fn build_arc(self) -> Arc<PoolAllocator<T>> {
        Arc::new(self.build().await)
    }
}

pub struct PoolItem<T>
where
    T: Send + Sync + 'static,
{
    resource: Option<Box<T>>,
    start_time: Instant,
    max_idling_timeout: Option<Duration>
}

impl<T> PoolItem<T> where T: Send + Sync + 'static
{
    pub fn new(resource: Box<T>, max_idling_timeout: Option<Duration>) -> Self {
        Self {
            resource: Some(resource),
            start_time: Instant::now(),
            max_idling_timeout
        }
    }

    pub fn is_empty(&self) -> bool {
       self.resource.is_none() 
    }

    pub fn take(&mut self) -> Option<Box<T>> {
        self.resource.take()
    }

    pub fn invalidate(&mut self) -> bool {
        if self.max_idling_timeout.is_some() && self.resource.is_some() {
            let elapsed = self.start_time.elapsed();
            if elapsed.gt(self.max_idling_timeout.as_ref().unwrap()) {
                drop(self.resource.take());
                return true;
            }
        }

        return false;
    }
}

impl<T> PoolAllocator<T> where T: Send + Sync + 'static
{
    pub async fn init(&mut self)
    {
        tokio_scoped::scope(|scope| {
            for _i in 0..self.min_pool_size {
                let pool = self.pool.clone();
                let resource_provider = &self.resource_provider;
                scope.spawn(async move {
                    let resource = resource_provider.new().await;
                    let pool_item: PoolItem<T> = PoolItem::new(Box::new(resource), None);

                    let mut pool = pool.lock().await;
                    pool.push_back(pool_item);
                });
            }
        });

        let pool = self.pool.lock().await;
        *self.current_pool_size.lock().await = pool.len();
        info!(
            target: "pool-allocator",
            "Initialized {} resources to fit the min_pool_size={}",
            pool.len(),
            self.min_pool_size
        );
    }

    pub async fn retrieve(&self) -> Result<Box<T>, Option<oneshot::Receiver<Box<T>>>> {
        let available_item = self.pool.lock().await.pop_front();
        if available_item.is_some() {
            return Ok(available_item.unwrap().take().unwrap()); 
        }

        // Note: The pool will be lock entirely until the resource
        // is inititalized.
        let mut current_pool_size_write = self.current_pool_size.lock().await;
        if *current_pool_size_write < self.max_pool_size {
           let resource = self.resource_provider.new().await;
           *current_pool_size_write += 1;
           return Ok(Box::new(resource))
        }

        drop(current_pool_size_write);
        let (sender, receiver) = oneshot::channel::<Box<T>>();
        self.waiters.lock().await.push_back(sender);
        Err(Some(receiver))
    }

    pub async fn put(&self, resource: Box<T>) {
        let resource = 'serve_waiter_first: {
            let mut waiters = self.waiters.lock().await;
            let waiter = waiters.pop_front();
            drop(waiters);

            if let Some(waiter) = waiter {
                if let Err(returned_resource) = waiter.send(resource) {
                    break 'serve_waiter_first Some(returned_resource)
                }

                break 'serve_waiter_first None
            }

            Some(resource)
        };

        if resource.is_none() {
            return
        }

        let mut pool = self.pool.lock().await;
        if pool.len() < self.max_pool_size {
            pool.push_back(PoolItem::new(resource.unwrap(), Some(self.resource_idle_timeout)));
        }
    }
}

