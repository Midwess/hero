use n0_future::task::spawn;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use super::allocator::{PoolAllocator, PoolItem};
use crate::maybe::MaybeSendSync;

pub struct PoolResponse<T>
where
    T: MaybeSendSync + 'static,
{
    resource: Option<PoolItem<T>>,
    pool: Option<Arc<PoolAllocator<T>>>,
}

impl<T> PoolResponse<T>
where
    T: MaybeSendSync + 'static,
{
    pub fn new(resource: PoolItem<T>, pool: Arc<PoolAllocator<T>>) -> Self {
        Self {
            resource: Some(resource),
            pool: Some(pool),
        }
    }
}

impl<T> TryInto<PoolItem<T>> for PoolResponse<T>
where
    T: MaybeSendSync,
{
    type Error = ();

    fn try_into(mut self) -> Result<PoolItem<T>, Self::Error> {
        match self.resource.take() {
            Some(resource) => Ok(resource),
            None => Err(()),
        }
    }
}

impl<T> Deref for PoolResponse<T>
where
    T: MaybeSendSync + 'static,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.resource.as_ref().expect("Cannot access returned resource").deref()
    }
}

impl<T> DerefMut for PoolResponse<T>
where
    T: MaybeSendSync + 'static,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.resource.as_mut().expect("Cannnot access the returned resource").deref_mut()
    }
}

impl<T> Drop for PoolResponse<T>
where
    T: MaybeSendSync + 'static,
{
    fn drop(&mut self) {
        let pool = self.pool.take();
        let resource = self.resource.take();
        if resource.is_none() {
            return;
        }

        if let (Some(pool), Some(resource)) = (pool, resource) {
            spawn(async move {
                pool.put(resource).await;
            });
        }
    }
}
