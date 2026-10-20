use futures::channel::oneshot;
use futures_timer::Delay;
use futures_util::future::join_all;
use futures_util::lock::Mutex;
use log::*;
use n0_future::task::{spawn, JoinHandle};
use n0_future::time::Instant;
use std::borrow::BorrowMut;
use std::cmp::min;
use std::collections::VecDeque;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

use super::response::PoolResponse;
use crate::maybe::MaybeSendSync;

#[derive(Debug, Error, PartialEq, Eq)]
/// Invalid pool configuration detected during construction.
pub enum PoolBuildError {
    /// The lower bound cannot be satisfied without exceeding the upper bound.
    #[error("minimum pool size {min} exceeds maximum pool size {max}")]
    InvalidSize {
        /// Requested number of permanently retained resources.
        min: usize,
        /// Requested maximum number of live resources.
        max: usize,
    },
}

/// Shared storage and capacity accounting behind a [`PoolAllocator`].
///
/// Most callers interact with [`PoolBuilder`] and
/// [`crate::pool::request::PoolRequest`] instead of manipulating this type.
pub struct Pool<T>
where
    T: MaybeSendSync + 'static,
{
    max_size: usize,
    min_size: usize,
    items: Mutex<VecDeque<PoolItem<T>>>,
    counter: Mutex<usize>,
}

impl<T> Pool<T>
where
    T: MaybeSendSync + 'static,
{
    fn new(min_size: usize, max_size: usize) -> Result<Self, PoolBuildError> {
        if min_size > max_size {
            return Err(PoolBuildError::InvalidSize {
                min: min_size,
                max: max_size,
            });
        }

        Ok(Self {
            items: Mutex::new(VecDeque::with_capacity(min_size)),
            counter: Mutex::new(0),
            min_size,
            max_size,
        })
    }

    async fn add_new_item(&self, item: PoolItem<T>) {
        if self.increase_counter().await.is_ok() {
            self.items.lock().await.push_front(item);
        }
    }

    async fn borrow_item(&self) -> Option<PoolItem<T>> {
        self.items.lock().await.pop_front()
    }

    async fn return_borrowed_item(&self, mut item: PoolItem<T>) {
        let mut counter = self.counter.lock().await;

        if *counter <= self.max_size {
            item.borrow_mut().refresh();
            drop(counter);
            self.items.lock().await.push_front(item);
        } else {
            *counter -= 1;
            // Discard orphan resource
            // This only occured when there is an item
            // that not belong to this pool.
        }
    }

    async fn increase_counter(&self) -> Result<usize, usize> {
        let mut counter = self.counter.lock().await;

        if *counter < self.max_size {
            *counter += 1;
            return Ok(*counter);
        }

        Err(*counter)
    }

    async fn decrease_counter(&self) -> Result<usize, usize> {
        let mut counter = self.counter.lock().await;
        if *counter > self.min_size {
            *counter -= 1;
            return Ok(*counter);
        }

        Err(*counter)
    }

    async fn invalidate(&self, index: usize) -> Result<(), Duration> {
        let mut items = self.items.lock().await;
        let Some(item) = items.get(index) else {
            return Ok(());
        };

        let timeleft = item.timeleft();
        if !timeleft.is_zero() {
            return Err(timeleft);
        }

        if let Some(removed_item) = items.swap_remove_back(index) {
            drop(items);
            if self.decrease_counter().await.is_err() {
                self.items.lock().await.push_back(removed_item);
            }
        }

        Ok(())
    }

    async fn number_of_available_items(&self) -> usize {
        self.items.lock().await.len()
    }

    async fn is_exceed_min_size(&self) -> bool {
        *self.counter.lock().await > self.min_size
    }

    /// Waits until every checkout has returned and only the minimum remains.
    ///
    /// This is primarily a shutdown/testing primitive. It has no timeout and
    /// will wait forever if a response guard is leaked.
    pub async fn wait_for_idle(&self) {
        loop {
            if self.min_size == *self.counter.lock().await && self.min_size == self.number_of_available_items().await {
                break;
            }

            Delay::new(Duration::from_millis(1)).await;
        }
    }
}

#[cfg_attr(not(target_family = "wasm"), async_trait::async_trait)]
#[cfg_attr(target_family = "wasm", async_trait::async_trait(?Send))]
/// Creates resources when the pool is empty but remains below capacity.
///
/// Creation failures should be represented in `T` or handled by the provider
/// because the pool assumes every call produces a usable value.
pub trait PoolResourceProvider<T>: MaybeSendSync
where
    T: MaybeSendSync,
{
    /// Creates one resource for immediate checkout or idle storage.
    async fn create(&self) -> T
    where
        Self: 'async_trait;
}

struct PoolCleaner<T>
where
    T: MaybeSendSync + 'static,
{
    pool: Arc<Pool<T>>,
    background_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl<T> PoolCleaner<T>
where
    T: MaybeSendSync,
{
    fn new(pool: Arc<Pool<T>>) -> Self {
        Self {
            pool,
            background_handle: Arc::new(Mutex::new(None)),
        }
    }

    async fn request_cleanup_loop(&self) {
        if !self.pool.is_exceed_min_size().await {
            return;
        }

        let mut background_handle = self.background_handle.lock().await;
        if background_handle.is_some() {
            return;
        }

        let pool = self.pool.clone();
        *background_handle = {
            let background_handle = self.background_handle.clone();
            Some(spawn(async move {
                let pool = pool;
                let mut curr_index = pool.min_size;
                let mut min_timeout: Option<Duration> = None;

                loop {
                    // yield first to make sure every lock can be quickly released
                    Delay::new(Duration::from_millis(1)).await;

                    if !pool.is_exceed_min_size().await {
                        break;
                    }

                    let number_of_idle_items = pool.number_of_available_items().await;

                    if number_of_idle_items == 0 {
                        break;
                    }

                    if curr_index >= number_of_idle_items {
                        if let Some(timeout) = min_timeout {
                            min_timeout = None;
                            Delay::new(timeout).await;
                        }

                        curr_index = 0;
                        continue;
                    }

                    if let Err(timeleft) = pool.invalidate(curr_index).await {
                        min_timeout = Some(min_timeout.map(|it| min(timeleft, it)).unwrap_or(timeleft));
                    }

                    curr_index += 1;
                }

                background_handle.lock().await.take();
            }))
        };
    }
}

impl<T> Drop for PoolCleaner<T>
where
    T: MaybeSendSync,
{
    fn drop(&mut self) {
        let background_handle = self.background_handle.clone();
        spawn(async move {
            let mut handle = background_handle.lock().await;
            if let Some(handle) = handle.as_mut() {
                handle.abort();
            }
        });
    }
}

/// Coordinates pooled resources, pending borrowers, and idle cleanup.
pub struct PoolAllocator<T>
where
    T: 'static + MaybeSendSync,
{
    /// Underlying capacity and idle-resource storage.
    pub pool: Arc<Pool<T>>,
    resource_idle_timeout: Duration,
    resource_provider: Box<dyn PoolResourceProvider<T> + 'static>,
    waiters: Mutex<VecDeque<oneshot::Sender<PoolResponse<T>>>>,

    cleaner: PoolCleaner<T>,
}

/// Configures and initializes a [`PoolAllocator`].
pub struct PoolBuilder<T>
where
    T: MaybeSendSync + 'static,
{
    min_pool_size: usize,
    max_pool_size: usize,
    resource_idle_timeout: Duration,
    pool_resource_provider: Box<dyn PoolResourceProvider<T> + 'static>,
}

impl<T> PoolBuilder<T>
where
    T: MaybeSendSync + 'static,
{
    /// Starts with a minimum of 10 resources, a maximum of 20, and a ten-minute
    /// idle timeout for resources above the minimum.
    pub fn new(resource_provider: Box<dyn PoolResourceProvider<T> + 'static>) -> Self {
        Self {
            min_pool_size: 10,
            max_pool_size: 20,
            resource_idle_timeout: Duration::new(60 * 10, 0),
            pool_resource_provider: resource_provider,
        }
    }

    /// Sets how long excess idle resources remain reusable.
    pub fn resource_idle_timeout(mut self, timeout: Duration) -> Self {
        self.resource_idle_timeout = timeout;

        self
    }

    /// Sets the number of resources created eagerly and retained while idle.
    pub fn min_pool_size(mut self, size: usize) -> Self {
        self.min_pool_size = size;

        self
    }

    /// Sets the maximum number of resources that may exist at once.
    pub fn max_pool_size(mut self, size: usize) -> Self {
        self.max_pool_size = size;
        self
    }

    /// Validates capacity, creates the minimum resources concurrently, and
    /// returns a shareable allocator.
    pub async fn build(self) -> Result<Arc<PoolAllocator<T>>, PoolBuildError> {
        let pool = Arc::new(Pool::new(self.min_pool_size, self.max_pool_size)?);

        let cleaner = PoolCleaner::new(pool.clone());

        let pool_allocator = PoolAllocator {
            resource_provider: self.pool_resource_provider,
            resource_idle_timeout: self.resource_idle_timeout,
            pool,
            cleaner,
            waiters: Mutex::new(VecDeque::new()),
        };

        pool_allocator.init().await;

        Ok(Arc::new(pool_allocator))
    }
}

/// A resource plus the timestamp used for excess-idle cleanup.
///
/// `Deref` and `DerefMut` expose the resource directly.
pub struct PoolItem<T>
where
    T: MaybeSendSync + 'static,
{
    resource: Box<T>,
    start_time: Instant,
    max_idling_timeout: Duration,
}

impl<T> PoolItem<T>
where
    T: MaybeSendSync + 'static,
{
    /// Wraps a newly created resource and starts its idle clock.
    pub fn new(resource: Box<T>, max_idling_timeout: Duration) -> Self {
        Self {
            resource,
            start_time: Instant::now(),
            max_idling_timeout,
        }
    }

    /// Restarts the idle clock when a resource returns to the pool.
    pub fn refresh(&mut self) {
        self.start_time = Instant::now();
    }

    /// Returns the remaining idle lifetime, saturating at zero.
    pub fn timeleft(&self) -> Duration {
        let elapsed = self.start_time.elapsed();
        if elapsed.gt(&self.max_idling_timeout) {
            return Duration::ZERO;
        }

        self.max_idling_timeout - elapsed
    }
}

impl<T> Deref for PoolItem<T>
where
    T: MaybeSendSync + 'static,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.resource
    }
}

impl<T> DerefMut for PoolItem<T>
where
    T: MaybeSendSync + 'static,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.resource.borrow_mut()
    }
}

impl<T> PoolAllocator<T>
where
    T: MaybeSendSync + 'static,
{
    /// Fills the pool to its configured minimum size.
    ///
    /// [`PoolBuilder::build`] calls this automatically. Calling it again only
    /// adds resources while capacity remains.
    pub async fn init(&self) {
        if self.pool.min_size == 0 {
            return;
        }

        let mut futures = vec![];

        for _i in 0..self.pool.min_size {
            let pool = self.pool.clone();
            let resource_provider = &self.resource_provider;
            let idle_timeout = self.resource_idle_timeout;
            futures.push(async move {
                let resource = resource_provider.create().await;
                let pool_item: PoolItem<T> = PoolItem::new(Box::new(resource), idle_timeout);

                pool.add_new_item(pool_item).await;
            });
        }

        join_all(futures).await;

        info!(
            target: "pool-allocator",
            "Initialized {} resources to fit the min_pool_size={}",
            self.pool.number_of_available_items().await,
            self.pool.min_size
        );
    }

    /// Checks out a resource or returns a receiver queued for the next return.
    ///
    /// The lower-level receiver form lets [`crate::pool::request::PoolRequest`]
    /// add timeout behavior without allocating a second queue.
    pub async fn retrieve(self: &Arc<Self>) -> Result<PoolResponse<T>, oneshot::Receiver<PoolResponse<T>>> {
        let mut waiters = self.waiters.lock().await;
        let available_item = self.pool.borrow_item().await;
        if let Some(available_item) = available_item {
            return Ok(PoolResponse::new(available_item, self.clone()));
        }

        // If there is not enough resources, we will create a new one until
        // it reached max_pool_size limit.
        if self.pool.increase_counter().await.is_ok() {
            let new_resource = self.resource_provider.create().await;
            let borrowed_item = PoolItem::new(Box::new(new_resource), self.resource_idle_timeout);

            return Ok(PoolResponse::new(borrowed_item, self.clone()));
        }

        // If the pool still not have enough available resources
        // we will init a waiter queue to wait for others resource return.
        let (sender, receiver) = oneshot::channel::<PoolResponse<T>>();
        waiters.push_back(sender);
        Err(receiver)
    }

    /// Returns a resource, preferring the oldest pending borrower.
    ///
    /// When no waiter accepts it, the resource becomes idle and may trigger
    /// cleanup if the pool is above its minimum size.
    pub async fn put(self: &Arc<Self>, resource: PoolItem<T>) {
        let mut waiters = self.waiters.lock().await;
        let resource = 'serve_waiter_first: {
            let mut resource = Some(resource);
            while let Some(waiter) = waiters.pop_front() {
                let Some(item) = resource.take() else {
                    break;
                };
                resource = waiter
                    .send(PoolResponse::new(item, self.clone()))
                    .err()
                    .and_then(|response| response.try_into().ok());

                if resource.is_none() {
                    break 'serve_waiter_first None;
                }
            }

            resource
        };

        if let Some(resource) = resource {
            self.pool.return_borrowed_item(resource).await;
            self.cleaner.request_cleanup_loop().await;
        }
    }
}
