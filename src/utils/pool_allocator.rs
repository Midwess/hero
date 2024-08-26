use std::{ops::Deref, sync::Arc, time::{Duration, Instant}};
use tokio::{runtime::Runtime, sync::Mutex};

pub trait PoolItem<P> where P: Send + Sync {
    fn new(param: &P) -> impl std::future::Future<Output = Self>;
}

pub struct PoolRequest<T, P> where T: PoolItem<P>, P: Send + Sync {
    pub pool: Arc<Mutex<PoolAllocator<T, P>>>,
}

impl<T, P> Clone for PoolRequest<T, P> where T: PoolItem<P>,  P: Send + Sync {
    fn clone(&self) -> Self {
        Self { pool: self.pool.clone() }
    }
}

pub struct PoolResponse<T, P> where T: PoolItem<P>,  P: Send + Sync {
    resource: Option<T>,
    pool: Arc<Mutex<PoolAllocator<T, P>>>
}

impl<T, P> Deref for PoolResponse<T, P> where T: PoolItem<P>,  P: Send + Sync {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.resource.as_ref().expect("Cannot access returned resource")
    }
}

impl<T, P> PoolRequest<T, P> where T: PoolItem<P>,  P: Send + Sync {
    pub async fn retreive(&self) -> PoolResponse<T, P> {
        let mut pool = self.pool.lock().await;

        let resource = pool.retrieve().await;

        PoolResponse {
            resource,
            pool: self.pool.clone()
        }
    }
}

impl<T, P> Drop for PoolResponse<T, P> where T: PoolItem<P>, P: Send + Sync {
    fn drop(&mut self) {
        let rt  = Runtime::new().unwrap();

        rt.block_on(async {
            self.pool.lock().await.put(self.resource.take().unwrap()).await;
        });
    }
}

#[derive(Clone)]
pub struct PoolAllocator <T, P> where T: PoolItem<P>,  P: Send + Sync {
    pub pool: Arc<Mutex<Vec<Option<T>>>>,
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
           pool: Arc::new(Mutex::new(pool)),
           max_pool_size: max_size,
           min_pool_size: min_size,
           retreiving_timeout,
           new_resource_param
       }
    }

    pub async fn retrieve(&mut self) -> Option<T> {
        let (resource, len): (Option<T>, usize) = {
            let mut pool = self.pool.lock().await;
            let size = pool.len();
            
            let found_available_index = pool.iter().position(|it| it.is_some());
            if found_available_index.is_some() {
                let resource = std::mem::replace(&mut pool[found_available_index.unwrap()], None);
                (resource, size)
            }
            else {
                (Option::<T>::None, size)
            }
        };

        match resource {
           Some(resource) => {
               return Some(resource)
           },
           None => {
                if len < self.max_pool_size as usize {
                    self.put(T::new(&self.new_resource_param).await).await;
                    return self.retrieve().await
                }

                let start_time = Instant::now();
                let mut elapsed = Duration::new(0, 0);
                while elapsed.lt(&self.retreiving_timeout) {
                    elapsed = start_time.elapsed();

                    let mut pool = self.pool.lock().await;
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
        let mut pool = self.pool.lock().await;
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

