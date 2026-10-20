//! Temporary ownership transfer with automatic return-on-drop.
//!
//! [`YieldContainer::retrieve`] removes the value and returns a [`Yieldable`]
//! guard. Dropping that guard schedules the value back into the container. This
//! is useful when an API needs exclusive ownership rather than a borrowed lock
//! guard across asynchronous work.

use futures_timer::Delay;
use futures_util::lock::Mutex;
use n0_future::future::block_on;
use n0_future::task::spawn;
use n0_future::time::Instant;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Weak};
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
/// Invalid ownership transition for a yield container.
pub enum YieldError {
    /// A value was deposited while another value was already available.
    #[error("The container is already empty")]
    AlreadyYielded,
    /// Retrieval found no currently available value.
    #[error("The container is empty")]
    NotYielded,
}

#[derive(Clone)]
/// Shared slot that lends full ownership of one value at a time.
pub struct YieldContainer<T: Send + 'static> {
    /// Shared storage used by this container and outstanding guards.
    pub value_container: Arc<Mutex<Option<Yieldable<T>>>>,
}

impl<T: Send + 'static> YieldContainer<T> {
    /// Creates a container with a value immediately available.
    pub fn new(value: T) -> Self {
        let container = Arc::new(Mutex::new(None));
        let yieldable = Yieldable {
            value: Some(value),
            container: Arc::downgrade(&container),
        };

        {
            let mut lock = block_on(container.lock());
            *lock = Some(yieldable);
        }

        Self {
            value_container: container,
        }
    }

    /// Creates an empty container that must be filled with [`Self::deposit`].
    pub fn empty() -> Self {
        Self {
            value_container: Arc::new(Mutex::new(None)),
        }
    }

    /// Makes a value available if the slot is currently empty.
    pub async fn deposit(&self, value: T) -> Result<(), YieldError> {
        let mut lock = self.value_container.lock().await;
        if lock.is_some() {
            return Err(YieldError::AlreadyYielded);
        }
        let container = Arc::downgrade(&self.value_container);
        *lock = Some(Yieldable {
            value: Some(value),
            container,
        });
        Ok(())
    }

    /// Removes the available value and transfers ownership to a guard.
    pub async fn retrieve(&self) -> Result<Yieldable<T>, YieldError> {
        let mut lock = self.value_container.lock().await;
        match lock.take() {
            Some(yieldable) => Ok(yieldable),
            None => Err(YieldError::NotYielded),
        }
    }

    /// Polls for an available value until the timeout elapses.
    ///
    /// Availability is checked every 10 milliseconds. A zero timeout still
    /// performs one immediate check.
    pub async fn retrieve_timed(&self, timeout: Duration) -> Result<Yieldable<T>, YieldError> {
        let start = Instant::now();
        loop {
            {
                let mut lock = self.value_container.lock().await;
                if let Some(yieldable) = lock.take() {
                    return Ok(yieldable);
                }
            }

            if start.elapsed() > timeout {
                return Err(YieldError::NotYielded);
            }

            Delay::new(Duration::from_millis(10)).await;
        }
    }
}

/// Exclusive value guard returned by [`YieldContainer::retrieve`].
///
/// Dropping the guard returns the value asynchronously when the originating
/// container still exists.
pub struct Yieldable<T: Send + 'static> {
    /// Owned value while it is checked out.
    pub value: Option<T>,
    container: Weak<Mutex<Option<Yieldable<T>>>>,
}

impl<T: Send + 'static> Deref for Yieldable<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value.as_ref().expect("Already yield back")
    }
}

impl<T: Send + 'static> DerefMut for Yieldable<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value.as_mut().expect("Already yield back")
    }
}

impl<T: Send + 'static> Drop for Yieldable<T> {
    fn drop(&mut self) {
        if let (Some(value), Some(container)) = (self.value.take(), self.container.upgrade()) {
            spawn(async move {
                let mut lock = container.lock().await;
                *lock = Some(Yieldable {
                    value: Some(value),
                    container: Arc::downgrade(&container),
                });
            });
        }
    }
}
