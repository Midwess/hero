use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Weak};
use futures_util::lock::Mutex;
use n0_future::task::spawn;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum YieldError {
    #[error("The container is already empty")]
    AlreadyYielded,
    #[error("The container is empty")]
    NotYielded,
}

pub struct YieldContainer<T: Send + 'static> {
    pub value_container: Arc<Mutex<Option<Yieldable<T>>>>,
}

impl<T: Send + 'static> YieldContainer<T> {
    pub fn new(value: T) -> Self {
        let container = Arc::new(Mutex::new(None));
        let yieldable = Yieldable {
            value: Some(value),
            container: Arc::downgrade(&container),
        };

        {
            let mut lock = futures_executor::block_on(container.lock());
            *lock = Some(yieldable);
        }

        Self {
            value_container: container,
        }
    }

    /// Retrieve ownership of Yieldable, removing it from container.
    pub async fn retrieve(&self) -> Result<Yieldable<T>, YieldError> {
        let mut lock = self.value_container.lock().await;
        match lock.take() {
            Some(yieldable) => Ok(yieldable),
            None => Err(YieldError::NotYielded),
        }
    }
}

pub struct Yieldable<T: Send + 'static> {
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
