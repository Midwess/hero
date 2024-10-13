use log::*;
use tokio::spawn;
use tokio::task::{yield_now, JoinHandle};
use std::borrow::BorrowMut;
use std::collections::VecDeque;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{oneshot, Mutex};

struct Pool<T> where T: Send + Sync + 'static {
    max_size: usize,
    min_size: usize,
    items: Mutex<VecDeque<PoolItem<T>>>,
    pool_size: Mutex<usize>,
}

impl<T> Pool<T> where T: Send + Sync + 'static {
    fn new(min_size: usize, max_size: usize) -> Self {
        if min_size > max_size {
            panic!("The min size can not be greater than max size");
        }

        Self {
           items: Mutex::new(VecDeque::with_capacity(min_size)),
           pool_size: Mutex::new(0),
           min_size,
           max_size
        } 
    }

    async fn add_new_item(&self, item: PoolItem<T>) {
        let mut pool_size = self.pool_size.lock().await;
        if *pool_size <= self.max_size {
            self.items.lock().await.push_front(item);
            *pool_size += 1;
        }
    }

    // Take an item out without reducing pool_size.
    // Because the item 'promised that it' will be returned (Visit PoolResponse for more detailed).
    async fn take_will_return(&self) -> Option<PoolItem<T>> {
        self.items.lock().await.pop_front()
    }

    // Return the resource to the pool
    // No need to increased pool_size count since it's already counted
    async fn return_old_item(&self, mut item: PoolItem<T>) {
        let mut current_size = self.pool_size.lock().await;

        if *current_size <= self.max_size {
            item.borrow_mut().refresh();
            self.items.lock().await.push_front(item);
        }
        else {
            *current_size -= 1;
            // Discard orphan resource
        }
    }

    async fn invalidate(&self, index: usize) {
        let mut items = self.items.lock().await;
        let item = items.get_mut(index);
        if item.is_some() && item.unwrap().invalidate() {
            let mut pool_size = self.pool_size.lock().await;
            items.swap_remove_back(index);
            *pool_size -= 1;
        }
    }

    async fn number_of_available_items(&self) -> usize {
        self.items.lock().await.len()
    }

    async fn is_exceed_min_size(&self) -> bool {
        *self.pool_size.lock().await > self.min_size
    }
}

#[async_trait::async_trait]
pub trait PoolResourceProvider<T>: Send + Sync
    where T: Send + Sync
{
    async fn new(&self) -> T where Self: 'async_trait;
}

pub struct PoolCleaner<T> where T: Send + Sync + 'static {
    pool: Arc<Pool<T>>,
    background_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    is_cleanup_running: Arc<Mutex<bool>>,
}

impl<T> PoolCleaner<T> where T: Send + Sync {
    fn new(pool: Arc<Pool<T>>) -> Self {
        Self {
            pool, background_handle: Arc::new(Mutex::new(None)), is_cleanup_running: Arc::new(Mutex::new(false))
        }
    }

    async fn request_cleanup_loop(&self) {
        let mut acquire_running_status_until_job_is_start = self.is_cleanup_running.lock().await;
        let is_cleanup_running = *acquire_running_status_until_job_is_start;
        if is_cleanup_running {
            return
        }

        let pool = self.pool.clone();
        *self.background_handle.lock().await = {
            let background_job_update_running = self.is_cleanup_running.clone();
            Some(spawn(async move {
                let pool = pool;
                let mut curr_index = pool.min_size;

                loop {
                    // yield first to make sure every lock can be quickly released
                    yield_now().await;

                    if !pool.is_exceed_min_size().await {
                        *background_job_update_running.lock().await = false;
                        break;
                    }

                    let number_of_idle_items = pool.number_of_available_items().await;
                    if number_of_idle_items == 0 {
                        continue;
                    }

                    if curr_index >= number_of_idle_items {
                        curr_index = 0;
                    }

                    pool.invalidate(curr_index).await;
                    curr_index += 1;
                } 
            }))
        };

        *acquire_running_status_until_job_is_start = true;
    }
}

impl<T> Drop for PoolCleaner<T> where T: Send + Sync {
    fn drop(&mut self) {
        let background_handle = self.background_handle.clone();
        spawn(async move {
            let mut handle = background_handle.lock().await;
            if handle.is_some() {
                handle.as_mut().unwrap().abort();
            }
        });
    }
}

pub struct PoolAllocator<T>
where
    T: 'static + Send + Sync
{
    pool: Arc<Pool<T>>,
    min_pool_size: usize,
    max_pool_size: usize,
    resource_idle_timeout: Duration,
    resource_provider: Box<dyn PoolResourceProvider<T> + 'static>,
    waiters: Mutex<VecDeque<oneshot::Sender<PoolItem<T>>>>,

    cleaner: PoolCleaner<T>
}

pub struct PoolBuilder<T> where T: Send + Sync + 'static
{
    min_pool_size: usize,
    max_pool_size: usize,
    resource_idle_timeout: Duration,
    pool_resource_provider: Option<Box<dyn PoolResourceProvider<T> + 'static>>,
}

impl<T> PoolBuilder<T> where T: Send + Sync + 'static
{
    pub fn new(resource_provider: Box<dyn PoolResourceProvider<T> + 'static>) -> Self {
        Self {
            min_pool_size: 10,
            max_pool_size: 20,
            resource_idle_timeout: Duration::new(10, 0),
            pool_resource_provider: Some(resource_provider),
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

    pub async fn build(mut self) -> Arc<PoolAllocator<T>>
    {
        if self.max_pool_size < self.min_pool_size {
            panic!("The max pool size can not less than min pool size");
        }

        let pool = Arc::new(Pool::new(self.min_pool_size, self.max_pool_size));

        let cleaner = PoolCleaner::new(pool.clone());

        let mut pool_allocator = PoolAllocator {
            max_pool_size: self.max_pool_size,
            min_pool_size: self.min_pool_size,
            resource_provider: self.pool_resource_provider.take().expect("The resource_provider is required"),
            resource_idle_timeout: self.resource_idle_timeout,
            pool,
            cleaner,
            waiters: Mutex::new(VecDeque::new()),
        };

        pool_allocator.init().await;

        Arc::new(pool_allocator)
    }
}

pub struct PoolItem<T>
where
    T: Send + Sync + 'static,
{
    resource: Box<T>,
    start_time: Instant,
    max_idling_timeout: Duration
}

impl<T> PoolItem<T> where T: Send + Sync + 'static
{
    pub fn new(resource: Box<T>, max_idling_timeout: Duration) -> Self {
        Self {
            resource,
            start_time: Instant::now(),
            max_idling_timeout
        }
    }

    pub fn refresh(&mut self) {
        self.start_time = Instant::now();
    }

    pub fn invalidate(&mut self) -> bool {
       let elapsed = self.start_time.elapsed();
       elapsed.gt(&self.max_idling_timeout)
    }
}

impl<T> Deref for PoolItem<T> where T: Send + Sync + 'static {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.resource
    }
}

impl<T> DerefMut for PoolItem<T> where T: Send + Sync + 'static {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.resource.borrow_mut()
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
                let idle_timeout = self.resource_idle_timeout.clone();
                scope.spawn(async move {
                    let resource = resource_provider.new().await;
                    let pool_item: PoolItem<T> = PoolItem::new(Box::new(resource), idle_timeout);

                    pool.add_new_item(pool_item).await;
                });
            }
        });

        info!(
            target: "pool-allocator",
            "Initialized {} resources to fit the min_pool_size={}",
            self.pool.pool_size.lock().await,
            self.min_pool_size
        );
    }

    pub async fn retrieve(&self) -> Result<PoolItem<T>, Option<oneshot::Receiver<PoolItem<T>>>> {
        // Take an available resource
        let available_item = self.pool.take_will_return().await;
        if available_item.is_some() {
            return Ok(available_item.unwrap()); 
        }

        // If there is no available resources, and the pool has not exceed max_pool_size
        // we will create a new resource
        // that resource will be instantly provide to client without adding to the pool
        // because it will go back to the pool after it's complete the duty.
        let mut current_pool_size_write = self.pool.pool_size.lock().await;
        if *current_pool_size_write < self.max_pool_size {
            *current_pool_size_write += 1;
            drop(current_pool_size_write);

            let resource = self.resource_provider.new().await;
            return Ok(PoolItem::new(Box::new(resource), self.resource_idle_timeout))
        }

        // If the max_pool_size also exceeded
        // We add the request to a queue. Waiting for a new
        // resource to come.
        drop(current_pool_size_write);
        let (sender, receiver) = oneshot::channel::<PoolItem<T>>();
        self.waiters.lock().await.push_back(sender);
        Err(Some(receiver))
    }

    pub async fn put(&self, resource: PoolItem<T>) {
        // Serving the waiters queue first.
        // Only add resource to the pool if no request is waiting.
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

        self.pool.return_old_item(resource.unwrap()).await;

        // If the returned resource not belong to min_pool_size
        // Then we trigger a cleanup loop to cleanup
        // idle resources base on idle timeout.
        if self.pool.is_exceed_min_size().await {
            self.cleaner.request_cleanup_loop().await;
        }
    }
}

