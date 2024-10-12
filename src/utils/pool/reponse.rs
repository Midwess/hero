use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use tokio::spawn;

use super::allocator::PoolAllocator;

pub struct PoolResponse<T>
where
    T: Send + Sync + 'static,
{
    resource: Option<Box<T>>,
    pool: Option<Arc<PoolAllocator<T>>>
}

impl<T> PoolResponse<T>
where
    T: Send + Sync + 'static
{
    pub fn new(resource: Box<T>, pool: Arc<PoolAllocator<T>>) -> Self {
        Self {
            resource: Some(resource),
            pool: Some(pool)
        }
    }
}

impl<T> Deref for PoolResponse<T>
where
    T: Send + Sync + 'static,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.resource.as_ref().expect("Cannot access returned resource")
    }
}

impl<T> DerefMut for PoolResponse<T>
    where T: Send + Sync + 'static
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.resource.as_mut().expect("Cannnot access the returned resource")
    }
}

impl<T> Drop for PoolResponse<T>
where
    T: Send + Sync + 'static
{
    fn drop(&mut self) {
        let pool = self.pool.take();
        let resource = self.resource.take();

        spawn(async move {
            pool.expect("This response already dropped")
                .put(resource.expect("The response already dropped"))
                .await;
        });
    }
}
