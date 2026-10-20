use super::allocator::PoolAllocator;
use super::response::PoolResponse;
use crate::maybe::MaybeSendSync;
use futures_timer::Delay;
use futures_util::{select, FutureExt};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
/// Invalid checkout request configuration.
pub enum PoolRequestBuildError {
    /// No allocator was supplied to the builder.
    #[error("pool is required")]
    MissingPool,
}

/// A clonable checkout policy bound to one allocator.
pub struct PoolRequest<T>
where
    T: MaybeSendSync + 'static,
{
    /// Maximum time to wait when every resource is checked out.
    pub retrieving_timeout: Option<Duration>,
    /// Allocator from which resources are checked out.
    pub pool: Arc<PoolAllocator<T>>,
}

#[derive(Default)]
/// Builder for [`PoolRequest`].
pub struct PoolRequestBuilder<T>
where
    T: 'static + MaybeSendSync,
{
    /// Pending wait timeout.
    pub retrieving_timeout: Option<Duration>,
    /// Pending allocator.
    pub pool_sync: Option<Arc<PoolAllocator<T>>>,
}

impl<T> PoolRequestBuilder<T>
where
    T: MaybeSendSync + 'static,
{
    /// Starts a request that waits indefinitely once a pool is assigned.
    pub fn new() -> Self {
        Self {
            retrieving_timeout: None,
            pool_sync: None,
        }
    }

    /// Limits how long checkout waits at maximum capacity.
    pub fn retrieving_timeout(mut self, timeout: Duration) -> Self {
        self.retrieving_timeout = Some(timeout);
        self
    }

    /// Selects the allocator used by this request.
    pub fn pool(mut self, pool: Arc<PoolAllocator<T>>) -> Self {
        self.pool_sync = Some(pool);
        self
    }

    /// Freezes the checkout policy.
    pub fn build(mut self) -> Result<PoolRequest<T>, PoolRequestBuildError> {
        Ok(PoolRequest {
            retrieving_timeout: self.retrieving_timeout,
            pool: self.pool_sync.take().ok_or(PoolRequestBuildError::MissingPool)?,
        })
    }
}

impl<T> Clone for PoolRequest<T>
where
    T: MaybeSendSync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            retrieving_timeout: self.retrieving_timeout,
        }
    }
}

impl<T> PoolRequest<T>
where
    T: MaybeSendSync + 'static,
{
    /// Checks out a resource according to this request's wait policy.
    ///
    /// `None` means either the configured timeout elapsed or the allocator's
    /// waiter channel closed. Dropping the returned guard gives the resource to
    /// the next waiter or returns it to idle storage.
    pub async fn retrieve(&self) -> Option<PoolResponse<T>> {
        match (self.retrieving_timeout, self.pool.retrieve().await) {
            (_, Ok(resource)) => Some(resource),
            (Some(timeout), Err(waiter)) => {
                let value = select! {
                    msg = waiter.fuse() => Some(msg),
                    _ = Delay::new(timeout).fuse() => None,
                    complete => None,
                };

                match value {
                    Some(Ok(value)) => Some(value),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}
