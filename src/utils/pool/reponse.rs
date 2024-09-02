use std::ops::Deref;
use std::sync::Arc;
use tokio::spawn;

use super::allocator::{PoolAllocator, PoolResource};

pub struct PoolResponse<T, P,>
where
    T: PoolResource<P,> + 'static,
    P: Send + Sync + 'static,
{
    resource: Option<T,>,
    pool: Option<Arc<PoolAllocator<T, P,>,>,>,
}

impl<T, P,> PoolResponse<T, P,>
where
    T: PoolResource<P,>,
    P: Send + Sync,
{
    pub fn new(resource: T, pool: Arc<PoolAllocator<T, P,>,>,) -> Self {
        Self {
            resource: Some(resource,),
            pool: Some(pool,),
        }
    }

    pub fn get(&self,) -> Option<&T,> {
        self.resource.as_ref()
    }

    pub fn get_mut(&mut self,) -> Option<&mut T,> {
        self.resource.as_mut()
    }
}

impl<T, P,> Deref for PoolResponse<T, P,>
where
    T: PoolResource<P,>,
    P: Send + Sync,
{
    type Target = T;

    fn deref(&self,) -> &Self::Target {
        self.resource.as_ref().expect("Cannot access returned resource",)
    }
}

impl<T, P,> Drop for PoolResponse<T, P,>
where
    T: PoolResource<P,>,
    P: Send + Sync,
{
    fn drop(&mut self,) {
        let pool = self.pool.take();
        let resource = self.resource.take();

        spawn(async move {
            pool.expect("This response already dropped",)
                .put(resource.expect("The response already dropped",),)
                .await;
        },);
    }
}
