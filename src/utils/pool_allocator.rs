use std::{marker::PhantomData, ops::Deref, sync::Arc, time::{Duration, Instant}};
use tokio::{spawn, sync::RwLock};

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

pub trait PoolResource<P>: Send + Sync where P: Send + Sync {
    fn new(param: &P) -> impl std::future::Future<Output = Self>;
}

pub struct PoolRequest<T, P> where T: PoolResource<P> + 'static, P: Send + Sync {
    retreving_timeout: Option<Duration>,
    pub pool: Arc<PoolAllocator<T, P>>,
}

pub struct PoolRequestBuilder<T, P> where T: PoolResource<P>, P: Send + Sync {
    retreiving_timeout: Option<Duration>,
    pool_sync: Option<Arc<PoolAllocator<T, P>>>,
}

impl<T, P> PoolRequestBuilder<T, P> where T: PoolResource<P>, P: Send + Sync {
    pub fn new() -> Self {
        Self {
            retreiving_timeout: None,
            pool_sync: None
        }
    }

    pub fn retreiving_timeout(mut self, timeout: Duration) -> Self {
        self.retreiving_timeout = Some(timeout);
        self
    }

    pub fn pool(mut self, pool: PoolAllocator<T, P>) -> Self {
        self.pool_sync = Some(Arc::new(pool));
        self
    }

    pub fn build(mut self) -> PoolRequest<T, P> {
        PoolRequest {
            retreving_timeout: self.retreiving_timeout,
            pool: self.pool_sync.take().expect("Pool must be set")
        }
    }
}

impl<T, P> Clone for PoolRequest<T, P> where T: PoolResource<P>,  P: Send + Sync {
    fn clone(&self) -> Self {
        Self { pool: self.pool.clone(), retreving_timeout: self.retreving_timeout.clone() }
    }
}

pub struct PoolResponse<T, P> where T: PoolResource<P> + 'static,  P: Send + Sync + 'static {
    resource: Option<T>,
    pool: Option<Arc<PoolAllocator<T, P>>>
}

impl<T, P> Deref for PoolResponse<T, P> where T: PoolResource<P>,  P: Send + Sync {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.resource.as_ref().expect("Cannot access returned resource")
    }
}

impl<T, P> PoolRequest<T, P> where T: PoolResource<P>,  P: Send + Sync + 'static {
    pub async fn retreive(&self) -> Option<PoolResponse<T, P>> {
        let resource = self.pool.retrieve().await;

        if resource.is_none() {
            if self.retreving_timeout.is_none() {
                return None
            }

            // Wait for available resource within timeout
            let now = Instant::now();
            let mut elapsed = Duration::new(0, 0);

            while elapsed.lt(self.retreving_timeout.as_ref().unwrap()) {
                let resource = self.pool.retrieve().await;
                if resource.is_some() {
                    return Some(PoolResponse {
                        resource,
                        pool: Some(self.pool.clone())
                    })
                }

                elapsed = now.elapsed();
            }

            return None
        }

        Some(PoolResponse {
            resource,
            pool: Some(self.pool.clone())
        })
    }
}

impl<T, P> Drop for PoolResponse<T, P> where T: PoolResource<P>, P: Send + Sync {
    fn drop(&mut self) {
        let pool = self.pool.take();
        let resource = self.resource.take();

        spawn(async move {
            pool.expect("This response already dropped").put(resource.expect("The response already dropped")).await;
        });
    }
}

#[derive(Clone)]
pub struct PoolAllocator <T, P> where T: PoolResource<P>,  P: Send + Sync {
    pool: Arc<RwLock<Vec<PoolItem<T, P>>>>,
    min_pool_size: i8,
    max_pool_size: i8,
    resource_idle_timeout: Duration,
    new_resource_param: P,
}

pub struct PoolBuilder<P> where P: Send + Sync + 'static {
    min_pool_size: i8,
    max_pool_size: i8,
    resource_idle_timeout: Duration,
    new_parameter_resource: Option<P>
}

impl<P> PoolBuilder<P> where P: Send + Sync + 'static {
    pub fn new(pool_param: P) -> Self {
        Self {
            min_pool_size: 10,
            max_pool_size: 20,
            resource_idle_timeout: Duration::new(10, 0),
            new_parameter_resource: Some(pool_param)
        }
    }

    pub fn min_pool_size(mut self, size: i8) -> Self {
        if size <= 0 {
            panic!("The pool size must be positive");
        }

        self.min_pool_size = size;

        self
    }

    pub fn max_pool_size(mut self, size: i8) -> Self {
        self.max_pool_size = size;

        self
    }

    pub async fn build<T>(&mut self) -> PoolAllocator<T, P> where T: PoolResource<P> {
        let pool = Vec::with_capacity(self.min_pool_size as usize); 

        if self.max_pool_size < self.min_pool_size {
            panic!("The max pool size can not less than min pool size");
        }

        PoolAllocator {
            max_pool_size: self.max_pool_size,
            min_pool_size: self.min_pool_size,
            new_resource_param: self.new_parameter_resource.take().unwrap(),
            resource_idle_timeout: self.resource_idle_timeout,
            pool: Arc::new(RwLock::new(pool))
        }
    }
}

pub struct PoolItem<T, P> where T: PoolResource<P>, P: Send + Sync {
    _marker: PhantomData<P>,
    pub resource: Option<T>,
    pub start_time: Instant,
    pub max_idling_timeout: Option<Duration>
}

impl<T, P> Deref for PoolItem<T, P> where T: PoolResource<P>, P: Send + Sync {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.resource.as_ref().expect("The resource has been invalidated or not initialized before use")
    }
}

impl<T, P> Default for PoolItem<T, P> where T: PoolResource<P>, P: Send + Sync {
    fn default() -> Self {
        Self {
            resource: None,
            start_time: Instant::now(),
            _marker: PhantomData::default(),
            max_idling_timeout: None,
        }
    }
}

impl<T, P> PoolItem<T, P> where T: PoolResource<P>, P: Send + Sync {
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

    pub fn invalidate(&mut self) {
        if self.max_idling_timeout.is_some() {
            let elapsed = self.start_time.elapsed();
            if elapsed.gt(self.max_idling_timeout.as_ref().unwrap()) {
                drop(self.resource.take());
            }
        }
    }
}

impl<T, P> PoolAllocator<T, P> where T: PoolResource<P>,  P: Send + Sync {
    pub async fn init(&mut self) {
       let mut pool = self.pool.write().await;
       for _i in 0..self.min_pool_size  {
          pool.push(PoolItem::new(&self.new_resource_param, Some(self.resource_idle_timeout)).await) 
       }
    }

    pub async fn retrieve(&self) -> Option<T> {
        // Request the write to ensure that there is no dangling data
        let mut pool = self.pool.write().await;
        let size = pool.len();
            
        let found_available_index = pool.iter().position(|it| it.resource.is_some());
        if found_available_index.is_some() {
            let pool_item = pool.get_mut(found_available_index.unwrap()).expect("Panic! UnExpected result, the pool item must be exist");
            return Some(pool_item.take());
        }
        else {
            if size < self.max_pool_size as usize {
                let mut new_item = PoolItem::<T, P>::new(&self.new_resource_param, Some(self.resource_idle_timeout)).await;
                let resource = new_item.take();
                pool.push(new_item);
                Some(resource)
            }
            else {
                None
            }
        }
    }

    pub async fn put(&self, resource: T) {
        let mut pool = self.pool.write().await;
        let found_unavailable_index = pool.iter().position(|it| it.resource.is_none());
        if found_unavailable_index.is_some() {
            let available_pool_item = pool.get_mut(found_unavailable_index.unwrap()).expect("Panic ! A resource when leave pool must always have place to be returned to pool");
            available_pool_item.put(resource);
        }
        else {
            panic!("Panic ! pool cannot provide more resource than it can contains")
        }
    } 

    pub async fn invalidate(&self) {
        let mut pool = self.pool.write().await;
        for pool_item in pool.iter_mut() {
            pool_item.invalidate()
        }
    }
}

#[cfg(test)]
pub mod test {
    use crate::utils::pool_allocator::*;

    pub struct Db {}

    impl PoolResource<()> for Db {
        async fn new(param: &()) -> Self {
            Self {}
        }
    }

    #[tokio::test]
    pub async fn test_pool() {
        let builder = PoolBuilder::new(());
        let pool = builder
            .min_pool_size(4)
            .max_pool_size(2)
            .build::<Db>()
            .await;

        let requester = PoolRequestBuilder::new()
            .pool(pool)
            .retreiving_timeout(Duration::new(10, 0))
            .build();

        let response = requester.retreive().await;

        assert_eq!(response.is_some(), true);
        let response = requester.retreive().await;
        drop(response);
        let response = requester.retreive().await;
        assert_eq!(response.is_some(), true);
        let response = requester.retreive().await;
        //let response = requester.retreive().await;
        assert_eq!(response.is_some(), true);
        drop(response);
        //
    }
}

