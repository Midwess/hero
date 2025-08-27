use std::{
    error::Error,
    fmt,
    future::Future,
    pin::Pin,
    sync::{Arc, Weak},
    task::{Context, Poll},
};
use std::sync::atomic::AtomicBool;
use futures::task::AtomicWaker;

struct CancelState {
    waker: AtomicWaker,
}

#[derive(Clone, Debug)]
pub struct CancellationToken {
    state: Arc<CancelState>,
    is_canceled: Arc<AtomicBool>,
}

impl Drop for CancellationToken {
    fn drop(&mut self) {
        let strong = Arc::strong_count(&self.state);
        self.is_canceled.store(strong == 0, std::sync::atomic::Ordering::Relaxed);
        self.state.waker.wake();
    }
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
            is_canceled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn signal(&self) -> AbortSignal {
        AbortSignal {
            inner: Arc::downgrade(&self.state),
            is_canceled: Arc::downgrade(&self.is_canceled),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AbortSignal {
    inner: Weak<CancelState>,
    is_canceled: Weak<AtomicBool>,
}

impl Future for AbortSignal {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Some(is_canceled) = self.is_canceled.upgrade() else {
            return Poll::Ready(());
        };

        if is_canceled.load(std::sync::atomic::Ordering::Relaxed) {
            return Poll::Ready(());
        }

        match self.inner.upgrade() {
            Some(state) => {
                state.waker.register(cx.waker());
            }
            None => return Poll::Ready(()),
        };

        if is_canceled.load(std::sync::atomic::Ordering::Relaxed) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
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

pub trait AbortableExt: Future + Sized {
    async fn abort_with(self, signal: AbortSignal) -> Result<Self::Output, AbortError>;
}

impl<Fu> AbortableExt for Fu
where
    Fu: Future,
{
    async fn abort_with(self, signal: AbortSignal) -> Result<Self::Output, AbortError> {
        use futures::{future::FutureExt, select};

        let task = self.fuse();
        let sig = signal.fuse();

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

    #[test]
    fn only_aborts_after_all_tokens_dropped() {
        block_on(async {
            let token1 = CancellationToken::new();
            let token2 = token1.clone(); // second handle to same state
            let signal = token1.signal();

            let task = async {
                Delay::new(Duration::from_millis(200)).await;
                42
            };

            let abort_trigger = async {
                Delay::new(Duration::from_millis(50)).await;
                drop(token1); // one token dropped, but another still exists
                Delay::new(Duration::from_millis(50)).await;
                assert!(!token2.is_canceled.load(std::sync::atomic::Ordering::Relaxed));

                drop(token2); // now last token dropped -> should cancel
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

