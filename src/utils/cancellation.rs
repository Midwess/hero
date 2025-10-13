use std::{
    sync::{Arc, Weak, Mutex, atomic::{AtomicBool, Ordering}},
};
use std::fmt::Debug;
use n0_future::task::spawn;
use futures::task::AtomicWaker;

struct CancelState {
    cancelled: AtomicBool,
    waker: AtomicWaker,
    children: Mutex<Vec<Weak<CancelState>>>,
}

#[derive(Clone)]
pub struct CancellationToken {
    state: Arc<CancelState>,
}

impl Debug for CancellationToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancellationToken")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

impl PartialEq for CancellationToken {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for CancellationToken {}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            state: Arc::new(CancelState {
                cancelled: AtomicBool::new(false),
                waker: AtomicWaker::new(),
                children: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Returns true if this token or its parent was cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::SeqCst)
    }

    /// Cancel this token and recursively cancel all child tokens.
    pub fn cancel(&self) {
        if !self.state.cancelled.swap(true, Ordering::SeqCst) {
            self.state.waker.wake();
            let children = self.state.children.lock().unwrap();
            for child_weak in children.iter() {
                if let Some(child) = child_weak.upgrade() {
                    child.cancelled.store(true, Ordering::SeqCst);
                    child.waker.wake();
                }
            }
        }
    }

    /// Creates a new child token that is linked to this token.
    pub fn child_token(&self) -> Self {
        let child = CancellationToken::new();
        self.state.children.lock().unwrap().push(Arc::downgrade(&child.state));
        if self.is_cancelled() {
            child.cancel();
        }
        child
    }

    /// Returns a future that resolves when this token is cancelled.
    pub fn cancelled(&self) -> Cancelled {
        Cancelled {
            state: Arc::downgrade(&self.state),
        }
    }

    pub fn drop_guard(&self) -> DropGuard {
        DropGuard::new(self.clone())
    }
}

pub struct DropGuard {
    token: CancellationToken,
}

impl DropGuard {
    pub fn new(token: CancellationToken) -> Self {
        Self { token }
    }
}

impl Drop for DropGuard {
    fn drop(&mut self) {
        self.token.cancel();
    }
}

pub struct Cancelled {
    state: Weak<CancelState>,
}

impl Future for Cancelled {
    type Output = ();

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        if let Some(state) = self.state.upgrade() {
            if state.cancelled.load(Ordering::SeqCst) {
                std::task::Poll::Ready(())
            }
            else {
                state.waker.register(cx.waker());
                std::task::Poll::Pending
            }
        }
        else {
            std::task::Poll::Ready(())
        }
    }
}

use std::future::Future;
use std::time::Duration;
use futures_timer::Delay;
use futures_util::{select, FutureExt};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TaskErrors {
    #[error("Task cancelled")]
    Cancelled
}

pub trait FutureExtension: Future + Sized {
    fn with_cancel(self, cancellation: &CancellationToken) -> impl Future<Output = Result<Self::Output, TaskErrors>>;
}

impl<T: Future + Unpin> FutureExtension for T {
    async fn with_cancel(self, cancellation: &CancellationToken) -> Result<Self::Output, TaskErrors> {
        if cancellation.is_cancelled() {
            return Err(TaskErrors::Cancelled)
        }

        let mut self_ = self.fuse();
        let mut cancelled = cancellation.cancelled().fuse();
        select! {
            output = self_ => {
                Ok(output)
            },
            _ = cancelled => {
                Err(TaskErrors::Cancelled)
            }
        }
    }
}

pub trait CancellationTokenExt {
    fn timeout(duration: Duration) -> CancellationToken;
}

impl CancellationTokenExt for CancellationToken {
    fn timeout(duration: Duration) -> CancellationToken {
        let token = CancellationToken::new();
        let child = token.clone();

        spawn(async move {
            Delay::new(duration).await;
            child.cancel();
        });

        token
    }
}