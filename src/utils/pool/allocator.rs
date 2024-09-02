use log::*;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use super::cleanup::{CleanupStrategy, PoolCleanup};

#[async_trait::async_trait]
pub trait PoolResource<P>: Send + Sync
where
    P: Send + Sync
{
    async fn new(param: &P) -> Self
    where
        P: 'async_trait;
}

/// Pool to provide a set of the same resource such as Http connection, Database connection, Socket connection.
/// Can use to serve high concurrent requests
/// How does it work:
/// PoolBuilder creates a PoolAllocator which will accept:
/// - Your resource which required to impl trait PoolResource
/// - Your parameter during the initallization of resource
/// PoolRequestBuilder creates PoolRequest which will accept:
/// - A PoolAllocator
/// How it works:
/// - PoolRequest request the PoolAllocator to provide new source
/// - PoolAllocator will return:
///     - First available resource if found
///     - Init more available resource if not reach max pool size
/// - PoolRequest will:
///     - Return PoolResponse if PoolAllocator return a success response
///     - Wait for retreiving_timeout until PoolAllocator have new resource to return
/// - PoolResponse will:
///     - Return the resource to the pool when it is dropped, you can manually do that by calling
///     drop(response)
///     - PoolResponse is implement Deref to resource
pub struct PoolAllocator<T, P>
where
    T: PoolResource<P> + 'static + Send + Sync,
    P: Send + Sync + 'static
{
    pool: Arc<RwLock<Vec<PoolItem<T, P>>>>,
    min_pool_size: u16,
    max_pool_size: u16,
    resource_idle_timeout: Duration,
    new_resource_param: P,
    // Keep the reference, make sure the life time of Cleanup always belong to PoolAllocator
    pool_cleanup: PoolCleanup<T, P>
}

pub struct PoolBuilder<P>
where
    P: Send + Sync + 'static
{
    min_pool_size: u16,
    max_pool_size: u16,
    resource_idle_timeout: Duration,
    new_parameter_resource: Option<P>,
    cleanup_strategy: Option<CleanupStrategy>
}

impl<P> PoolBuilder<P>
where
    P: Send + Sync + 'static
{
    pub fn new(pool_param: P) -> Self {
        Self {
            min_pool_size: 10,
            max_pool_size: 20,
            resource_idle_timeout: Duration::new(10, 0),
            new_parameter_resource: Some(pool_param),
            cleanup_strategy: Some(CleanupStrategy::Best {
                interval: Duration::new(10, 0),
                min_pool_size: 10
            })
        }
    }

    pub fn min_pool_size(mut self, size: u16) -> Self {
        if size <= 0 {
            panic!("The pool size must be positive");
        }

        self.min_pool_size = size;

        self
    }

    pub fn max_pool_size(mut self, size: u16) -> Self {
        self.max_pool_size = size;

        self
    }

    pub fn cleanup(mut self, strategy: CleanupStrategy) -> Self {
        self.cleanup_strategy = Some(strategy);
        self
    }

    pub async fn build<T>(&mut self) -> Arc<PoolAllocator<T, P>>
    where
        T: PoolResource<P> + 'static,
        P: Send + Sync + 'static
    {
        let pool = Vec::with_capacity(0);

        if self.max_pool_size < self.min_pool_size {
            panic!("The max pool size can not less than min pool size");
        }

        let pool = Arc::new(RwLock::new(pool));

        let cleanup = PoolCleanup::new(pool.clone(), self.cleanup_strategy.take().unwrap());
        let mut pool_allocator = PoolAllocator {
            max_pool_size: self.max_pool_size,
            min_pool_size: self.min_pool_size,
            new_resource_param: self.new_parameter_resource.take().unwrap(),
            resource_idle_timeout: self.resource_idle_timeout,
            pool,
            pool_cleanup: cleanup
        };

        pool_allocator.init().await;

        Arc::new(pool_allocator)
    }
}

pub struct PoolItem<T, P>
where
    T: PoolResource<P>,
    P: Send + Sync
{
    _marker: PhantomData<P>,
    pub resource: Option<T>,
    pub start_time: Instant,
    pub max_idling_timeout: Option<Duration>
}

impl<T, P> Deref for PoolItem<T, P>
where
    T: PoolResource<P>,
    P: Send + Sync
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.resource.as_ref().expect("The resource has been invalidated or not initialized before use")
    }
}

impl<T, P> Default for PoolItem<T, P>
where
    T: PoolResource<P>,
    P: Send + Sync
{
    fn default() -> Self {
        Self {
            resource: None,
            start_time: Instant::now(),
            _marker: PhantomData::default(),
            max_idling_timeout: None
        }
    }
}

impl<T, P> PoolItem<T, P>
where
    T: PoolResource<P> + Send + Sync,
    P: Send + Sync
{
    pub async fn new(param: &P, max_idling_timeout: Option<Duration>) -> Self {
        let resource = T::new(param).await;
        Self {
            resource: Some(resource),
            start_time: Instant::now(),
            _marker: PhantomData::default(),
            max_idling_timeout
        }
    }

    pub fn take(&mut self) -> T {
        self.resource.take().unwrap()
    }

    pub fn put(&mut self, resource: T) {
        self.resource = Some(resource);
        self.start_time = Instant::now();
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

impl<T, P> PoolAllocator<T, P>
where
    T: PoolResource<P> + Send + Sync,
    P: Send + Sync
{
    pub async fn init(&mut self)
    where
        Self: Send
    {
        let params = &self.new_resource_param;
        tokio_scoped::scope(|scope| {
            for _i in 0..self.min_pool_size {
                let pool = self.pool.clone();
                scope.spawn(async move {
                    let pool_item: PoolItem<T, P> = PoolItem::new(params, None).await;

                    let mut pool = pool.write().await;
                    pool.push(pool_item);
                });
            }
        });

        let pool = self.pool.read().await;
        info!(
            target: "pool-allocator",
            "Initialized {} resources to fit the min_pool_size={}",
            pool.len(),
            self.min_pool_size
        );

        self.pool_cleanup.start_background();
    }

    pub async fn retrieve(&self) -> Option<T> {
        let result = {
            // Request the write to ensure that there is no dangling data
            let mut pool = self.pool.write().await;
            let size = pool.len();

            let found_available_index = pool.iter().position(|it| it.resource.is_some());
            if found_available_index.is_some() {
                let pool_item = pool
                    .get_mut(found_available_index.unwrap())
                    .expect("Panic! UnExpected result, the pool item must be exist");
                return Some(pool_item.take());
            } else {
                if size < self.max_pool_size as usize {
                    let mut new_item =
                        PoolItem::<T, P>::new(&self.new_resource_param, Some(self.resource_idle_timeout)).await;
                    let resource = new_item.take();
                    pool.push(new_item);
                    Some(resource)
                } else {
                    None
                }
            }
        };

        if result.is_some() {
            // if result is none => there is no connection left to free
            PoolCleanup::request_cleanup_loop(&self.pool, self.min_pool_size as usize).await;
        }

        result
    }

    pub async fn put(&self, resource: T) {
        let result = {
            let mut pool = self.pool.write().await;
            let found_unavailable_index = pool.iter().position(|it| it.resource.is_none());
            if found_unavailable_index.is_some() {
                let available_pool_item = pool
                    .get_mut(found_unavailable_index.unwrap())
                    .expect("Panic! A resource when leave pool must always have place to be returned to pool");
                available_pool_item.put(resource);
            } else {
                panic!("Panic ! pool cannot provide more resource than it can contains")
            }
        };

        PoolCleanup::request_cleanup_loop(&self.pool, self.min_pool_size as usize).await;
        result
    }
}
