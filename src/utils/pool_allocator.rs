use std::{ops::Deref, sync::Arc, thread::scope, time::{Duration, Instant}};
use tokio::{join, runtime::Runtime, spawn, sync::{Mutex, RwLock}, time::sleep};

pub trait PoolItem<P>: Send + Sync where P: Send + Sync {
    fn new(param: &P) -> impl std::future::Future<Output = Self>;
}

pub struct PoolRequest<T, P> where T: PoolItem<P> + 'static, P: Send + Sync {
    pub pool: Arc<PoolAllocator<T, P>>,
}

impl<T, P> Clone for PoolRequest<T, P> where T: PoolItem<P>,  P: Send + Sync {
    fn clone(&self) -> Self {
        Self { pool: self.pool.clone() }
    }
}

pub struct PoolResponse<T, P> where T: PoolItem<P> + 'static,  P: Send + Sync + 'static {
    resource: Option<T>,
    pool: Option<Arc<PoolAllocator<T, P>>>
}

impl<T, P> Deref for PoolResponse<T, P> where T: PoolItem<P>,  P: Send + Sync {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.resource.as_ref().expect("Cannot access returned resource")
    }
}

impl<T, P> PoolRequest<T, P> where T: PoolItem<P>,  P: Send + Sync + 'static {
    pub async fn retreive(&self) -> PoolResponse<T, P> {
        let resource = self.pool.retrieve().await;

        PoolResponse {
            resource,
            pool: Some(self.pool.clone())
        }
    }
}

impl<T, P> Drop for PoolResponse<T, P> where T: PoolItem<P>, P: Send + Sync {
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
pub struct PoolAllocator <T, P> where T: PoolItem<P>,  P: Send + Sync {
    pub pool: Arc<RwLock<Vec<Option<T>>>>,
    pub min_pool_size: i8,
    pub max_pool_size: i8,
    pub retreiving_timeout: Duration,
    pub new_resource_param: P
}

impl<T, P> PoolAllocator<T, P> where T: PoolItem<P>,  P: Send + Sync {
    pub async fn new(min_size: i8, max_size: i8, new_resource_param: P, retreiving_timeout: Duration) -> Self {
       let mut pool = Vec::with_capacity(max_size as usize); 
       for _i in 0..min_size  {
          pool.push(Some(T::new(&new_resource_param).await)) 
       };

       Self {
           pool: Arc::new(RwLock::new(pool)),
           max_pool_size: max_size,
           min_pool_size: min_size,
           retreiving_timeout,
           new_resource_param
       }
    }

    pub async fn retrieve(&self) -> Option<T> {
        let (resource, len): (Option<T>, usize) = {
            let mut pool = self.pool.write().await;
            let size = pool.len();
            
            let found_available_index = pool.iter().position(|it| it.is_some());
            if found_available_index.is_some() {
                let resource = std::mem::replace(&mut pool[found_available_index.unwrap()], None);
                (resource, size)
            }
            else {
                if size < self.max_pool_size as usize {
                    pool.push(None);
                }

                (Option::<T>::None, size)
            }
        };

        match resource {
           Some(resource) => {
               return Some(resource)
           },
           None => {
                if len < self.max_pool_size as usize {
                    return Some(T::new(&self.new_resource_param).await);
                }

                let start_time = Instant::now();
                let mut elapsed = Duration::new(0, 0);
                while elapsed.lt(&self.retreiving_timeout) {
                    elapsed = start_time.elapsed();

                    let mut pool = self.pool.write().await;
                    let found_available_index = pool.iter().position(|it| it.is_some());
                    if found_available_index.is_some() {
                        let resource = std::mem::replace(&mut pool[found_available_index.unwrap()], None);
                        return resource;
                    }
                }

                return None
           }
        }
    }

    pub async fn put(&self, pool_item: T) {
        let mut pool = self.pool.write().await;
        let found_unavailable_index = pool.iter().position(|it| it.is_none());
        if found_unavailable_index.is_none() {
            if pool.len() < self.max_pool_size as usize {
                pool.push(Some(pool_item))
            }

            return;
        }

        let _ = std::mem::replace(&mut pool[found_unavailable_index.unwrap()], Some(pool_item));
    } 
}

#[cfg(test)]
pub mod test {
    use std::sync::Arc;
    use crate::utils::pool_allocator::*;

    use tokio::sync::Mutex;
    use tokio::time::sleep;

    pub struct Db {}

    impl PoolItem<()> for Db {
        async fn new(param: &()) -> Self {
            Self {}
        }
    }

    #[tokio::test]
    pub async fn test_pool() {
        let pool = Arc::new(PoolAllocator::<Db, ()>::new(1, 2, (), Duration::new(10, 0)).await);
        let requester = PoolRequest {
            pool: pool.clone()
        };

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
        //
        //let response = requester.retreive().await;
        //assert_eq!(response.resource.is_some(), true);
        //
        //drop(response);
        //
    }
}

