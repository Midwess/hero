use std::{marker::PhantomData, sync::Arc, time::{Duration, Instant}};
use tokio::sync::RwLock;

use super::{allocator::{PoolAllocator, PoolResource}, reponse::PoolResponse};

pub struct PoolRequest<T, P> where T: PoolResource<P> + 'static, P: Send + Sync + 'static {
    _marker: PhantomData<T>,
    retreving_timeout: Option<Duration>,
    pub pool: Arc<PoolAllocator<T, P>>,
}

pub struct PoolRequestBuilder<T, P> where T: PoolResource<P> + 'static, P: Send + Sync + 'static {
    _marker: PhantomData<T>,
    retreiving_timeout: Option<Duration>,
    pool_sync: Option<Arc<PoolAllocator<T, P>>>,
}

impl<T, P> PoolRequestBuilder<T, P> where T: PoolResource<P>, P: Send + Sync {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
            retreiving_timeout: None,
            pool_sync: None
        }
    }

    pub fn retreiving_timeout(mut self, timeout: Duration) -> Self {
        self.retreiving_timeout = Some(timeout);
        self
    }

    pub fn pool(mut self, pool: Arc<PoolAllocator<T, P>>) -> Self {
        self.pool_sync = Some(pool);
        self
    }

    pub fn build(mut self) -> PoolRequest<T, P> {
        PoolRequest {
            _marker: PhantomData::default(),
            retreving_timeout: self.retreiving_timeout,
            pool: self.pool_sync.take().expect("Pool must be set")
        }
    }
}

impl<T, P> Clone for PoolRequest<T, P> where T: PoolResource<P>,  P: Send + Sync {
    fn clone(&self) -> Self {
        Self {
            _marker: PhantomData::default(),
            pool: self.pool.clone(),
            retreving_timeout: self.retreving_timeout.clone()
        }
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
                    return Some(PoolResponse::new(resource.unwrap(), self.pool.clone()));
                }

                elapsed = now.elapsed();
            }

            return None
        }

        let response = PoolResponse::new(resource.unwrap(), self.pool.clone());

        Some(response)
    }
}

