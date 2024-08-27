use std::{marker::PhantomData, ops::Deref, sync::Arc, thread::scope, time::{Duration, Instant}};
use tokio::{join, runtime::Runtime, spawn, sync::{Mutex, RwLock}, time::sleep};

/* 
 * Resource pool
 * A Pool is created using PoolBuilder which require two generic, one for type of parameter to use
 * as an input of creation, another is the resource type.
 * PoolRequest will keep reference to Pool through Arc, handle communication with pool
 * PoolRequest return a PoolResponse which will also keep reference to pool using Arc
 * After the PoolResponse were dropped it will return the resource back to the pool
 */

pub trait PoolResource<P>: Send + Sync where P: Send + Sync {
    fn new(param: &P) -> impl std::future::Future<Output = Self>;
}

pub struct PoolRequest<T, P> where T: PoolResource<P> + 'static, P: Send + Sync {
    retreving_timeout: Duration,
    pub pool: Arc<PoolAllocator<T, P>>,
}

pub struct PoolRequestBuilder<T, P> where T: PoolResource<P>, P: Send + Sync {
    retreiving_timeout: Duration,
    pool_sync: Option<Arc<PoolAllocator<T, P>>>,
}

impl<T, P> PoolRequestBuilder<T, P> where T: PoolResource<P>, P: Send + Sync {
    pub fn new() -> Self {
        Self {
            retreiving_timeout: Duration::new(20, 0),
            pool_sync: None
        }
    }

    pub fn retreiving_timeout(mut self, timeout: Duration) -> Self {
        self.retreiving_timeout = timeout;
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
    pub async fn retreive(&self) -> PoolResponse<T, P> {
        let resource = self.pool.retrieve().await;

        PoolResponse {
            resource,
            pool: Some(self.pool.clone())
        }
    }
}

impl<T, P> Drop for PoolResponse<T, P> where T: PoolResource<P>, P: Send + Sync {
    fn drop(&mut self) {
        let pool = self.pool.take();
        let resource = self.resource.take();

        spawn(async move {
            pool.expect("This response already dropped").put(resource.expect("The response already dropped")).await;
            println!("Returned 1 resource to pool in async");
        });
    }
}

#[derive(Clone)]
pub struct PoolAllocator <T, P> where T: PoolResource<P>,  P: Send + Sync {
    pub pool: Arc<RwLock<Vec<PoolItem<T, P>>>>,
    pub min_pool_size: i8,
    pub max_pool_size: i8,
    pub resource_idle_timeout: Duration,
    pub new_resource_param: P,
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
            _marker: PhantomData::default()
        }
    }
}

impl<T, P> PoolItem<T, P> where T: PoolResource<P>, P: Send + Sync {
    pub async fn new(param: &P) -> Self {
        let resource = T::new(param).await;
        Self {
            resource: Some(resource),
            _marker: PhantomData::default()
        }
    }

    pub fn take(&mut self) -> T {
        self.resource.take().unwrap()
    }

    pub fn put(&mut self, resource: T) {
        self.resource = Some(resource)
    }
}

impl<T, P> PoolAllocator<T, P> where T: PoolResource<P>,  P: Send + Sync {
    pub async fn init(&mut self) {
       let mut pool = self.pool.write().await;
       for _i in 0..self.min_pool_size  {
          pool.push(PoolItem::new(&self.new_resource_param).await) 
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
                let mut new_item = PoolItem::<T, P>::new(&self.new_resource_param).await;
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
}

#[cfg(test)]
pub mod test {
    use std::sync::Arc;
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
            .min_pool_size(1)
            .max_pool_size(2)
            .build::<Db>()
            .await;

        let requester = PoolRequestBuilder::new()
            .pool(pool)
            .build();

        let response = requester.retreive().await;
        println!("Requested 1");

        assert_eq!(response.resource.is_some(), true);
        let response = requester.retreive().await;
        println!("Requested 2");
        drop(response);
        let response = requester.retreive().await;
        assert_eq!(response.resource.is_some(), true);
        println!("Requested 3");
        drop(response);
        let response = requester.retreive().await;
        //let response = requester.retreive().await;
        assert_eq!(response.resource.is_some(), true);
        //
        //drop(response);
        //
    }
}

