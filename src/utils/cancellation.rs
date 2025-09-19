use std::{
    error::Error,
    fmt,
    future::Future,
    pin::Pin,
    sync::{Arc, Weak},
    task::{Context, Poll},
};
use futures::task::AtomicWaker;

struct CancelState {
    waker: AtomicWaker,
}

#[derive(Clone, Debug)]
pub struct CancellationToken {
    state: Arc<CancelState>,
    life_count: Arc<()>,
}

impl PartialEq for CancellationToken {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for CancellationToken {}

impl fmt::Debug for CancelState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CancelState").finish_non_exhaustive()
    }
}

impl Eq for CancelState {}

impl PartialEq for CancelState {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl CancellationToken {
    pub fn new() -> Self {
        CancellationToken {
            state: Arc::new(CancelState {
                waker: AtomicWaker::new(),
            }),
            life_count: Arc::new(()),
        }
    }

    pub fn signal(&self) -> AbortSignal {
        AbortSignal {
            inner: Arc::downgrade(&self.state),
            life_count: Arc::downgrade(&self.life_count),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AbortSignal {
    inner: Weak<CancelState>,
    life_count: Weak<()>,
}

impl Future for AbortSignal {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.life_count.strong_count() == 0 {
            return Poll::Ready(());
        }

        match self.inner.upgrade() {
            Some(state) => {
                state.waker.register(cx.waker());
                Poll::Pending
            }
            None => Poll::Ready(())
        }
    }
}

impl AbortSignal {
    pub fn is_aborted(&self) -> bool {
        self.life_count.strong_count() == 0
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AbortError {
    Cancelled,
}

impl fmt::Display for AbortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AbortError::Cancelled => write!(f, "The operation was cancelled"),
        }
    }
}

impl Error for AbortError {}

impl Into<AbortSignal> for &AbortSignal {
    fn into(self) -> AbortSignal {
        self.clone()
    }
}

pub trait AbortableExt: Future + Sized {
    async fn abort_with(self, signal: impl Into<AbortSignal>) -> Result<Self::Output, AbortError>;
}

impl<Fu> AbortableExt for Fu
where
    Fu: Future,
{
    async fn abort_with(self, signal: impl Into<AbortSignal>) -> Result<Self::Output, AbortError> {
        use futures::{future::FutureExt, select};

        let task = self.fuse();
        let sig = signal.into().fuse();

        futures::pin_mut!(task);
        futures::pin_mut!(sig);

        select! {
            res = task => Ok(res),
            _ = sig => Err(AbortError::Cancelled),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use futures_timer::Delay;
    use std::time::Duration;

    #[test]
    fn completes_without_abort() {
        block_on(async {
            let token = CancellationToken::new();
            let signal = token.signal();

            let task = async {
                Delay::new(Duration::from_millis(50)).await;
                123
            };

            let result = task.abort_with(signal).await;
            assert_eq!(result.unwrap(), 123);
        });
    }

    #[test]
    fn aborts_in_middle_of_task() {
        block_on(async {
            let token = CancellationToken::new();
            let signal = token.signal();

            let task = async {
                // Simulate some work that should be cut off early
                for i in 0..10 {
                    Delay::new(Duration::from_millis(100)).await;
                    println!("step {}", i);
                }
                999
            };

            // Trigger cancellation after a short delay
            let abort_trigger = async {
                Delay::new(Duration::from_millis(250)).await;
                drop(token); // dropping triggers abort
            };

            let (res, _) = futures::join!(
                async {
                    task.abort_with(signal).await
                },
                abort_trigger,
            );

            assert_eq!(res.err().unwrap(), AbortError::Cancelled);
        });
    }
}
