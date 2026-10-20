//! Cancellation: one token per unit of work, linked into a tree, observed by
//! the work rather than enforced on it.
//!
//! A [`CancellationToken`] is a shared flag with waiters. Cloning shares the
//! flag; [`child_token`](CancellationToken::child_token) makes a new flag the
//! parent reaches when it cancels. A scope installs a token as the task-local
//! [`current`](CancellationToken::current) for the future it wraps and keeps
//! the enclosing one as [`parent`](CancellationToken::parent).
//!
//! Cancellation contract:
//!
//! - Cancelling is a signal, never a stop. [`cancel`](CancellationToken::cancel) sets the flag, wakes every waiter on
//!   [`cancelled`](CancellationToken::cancelled), and reaches every descendant, recursively. Work stops only where it
//!   looks: `is_cancelled`, `cancelled().await`, or [`with_cancel`](FutureExtension::with_cancel), which answers
//!   [`TaskErrors::Cancelled`] and drops the wrapped future, and answers it without polling when the token was already
//!   cancelled.
//! - Cancellation flows down, never up or sideways. A cancelled parent cancels its children and theirs; a cancelled
//!   child leaves its parent and siblings running. A child made under an already cancelled parent is born cancelled.
//! - A token cancels at most once and never clears. A second `cancel` does nothing.
//! - No wakeup is lost. A waiter registered before the flag flips is woken exactly once and finds it set; one
//!   registered after finds it set without waiting.
//! - Dropping every handle to a token resolves its `cancelled()` waiters as if it had been cancelled, and detaches its
//!   children: a parent holds them weakly, they hold nothing back, so they run on as roots, uncancelled.
//! - A scope is lexical to the future, not to the task. [`scope`](CancellationToken::scope) makes the token `current()`
//!   while the future is polled and restores the enclosing token when the future completes, panics, or is dropped; it
//!   never cancels the token it installed. Interleaved scopes on one thread keep their own tokens across awaits. A task
//!   spawned inside a scope with a runtime's own `spawn` inherits nothing;
//!   [`spawn_scoped`](CancellationToken::spawn_scoped) inherits by opening a child scope captured at the call.
//! - [`child_scope`](CancellationToken::child_scope) ties a child token's life to its future: the token is cancelled
//!   when the future completes, panics, or is dropped, polled or not. [`drop_guard`](CancellationToken::drop_guard) is
//!   the same rule held by hand; [`DropGuard::disarm`] lets go of it.
//! - Outside a scope there is no ambient token. `current()` and `child_scope` answer
//!   [`TaskErrors::MissingCancellationScope`]; [`current_or_new`](CancellationToken::current_or_new) hands out a fresh,
//!   independent root on every call and installs nothing.
//! - [`timeout`](CancellationToken::timeout) and [`cancel_after`](CancellationToken::cancel_after) cancel from a
//!   spawned timer. Dropping the handle `timeout` returned aborts its timer; clones carry no timer and never abort it.
//! - Equality says nothing: two tokens always compare equal, so containing types can derive it. Serialization carries
//!   nothing: a deserialized token is a fresh, uncancelled root.

use event_listener::{Event, EventListener};
use n0_future::task::{spawn, JoinHandle};
use parking_lot::Mutex;
use std::fmt::Debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

tokio::task_local! {
    static CANCELLATION_SCOPE: (CancellationToken, Option<CancellationToken>);
}

#[derive(Debug, Default)]
struct CancelState {
    cancelled: AtomicBool,
    event: Event,
    children: Mutex<Vec<Weak<CancelState>>>,
}

#[derive(Serialize, Deserialize)]
pub struct CancellationToken {
    #[serde(skip)]
    state: Arc<CancelState>,
    #[serde(skip)]
    timeout_handle: Option<JoinHandle<()>>,
}

impl Clone for CancellationToken {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            timeout_handle: None,
        }
    }
}

impl Debug for CancellationToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancellationToken").field("cancelled", &self.is_cancelled()).finish()
    }
}

impl PartialEq for CancellationToken {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for CancellationToken {}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for CancellationToken {
    fn drop(&mut self) {
        if let Some(handle) = self.timeout_handle.take() {
            handle.abort()
        }
    }
}

impl CancellationToken {
    /// Return the token installed by the active cancellation scope.
    pub fn current() -> Result<Self, TaskErrors> {
        CANCELLATION_SCOPE
            .try_with(|(token, _)| token.clone())
            .map_err(|_| TaskErrors::MissingCancellationScope)
    }

    /// Return the active token, or create an independent token outside a scope.
    ///
    /// The fallback is not installed as a scope. Separate calls outside a scope
    /// return separate tokens.
    pub fn current_or_new() -> Self {
        Self::current().unwrap_or_default()
    }

    /// Return the enclosing scope's token, if this scope has a parent.
    ///
    /// This describes scope nesting, not the ancestry of an arbitrary token.
    pub fn parent() -> Option<Self> {
        CANCELLATION_SCOPE.try_with(|(_, parent)| parent.clone()).ok().flatten()
    }

    /// Install this token while the supplied future is polled or dropped.
    ///
    /// Captures the enclosing scope when called. Awaited calls see this token;
    /// independently spawned tasks must use `spawn_scoped` or an explicit scope.
    /// Exiting restores the enclosing scope without cancelling this token.
    ///
    /// ```
    /// use hero::cancellation::CancellationToken;
    ///
    /// # futures::executor::block_on(async {
    /// let token = CancellationToken::current_or_new();
    /// token.scope(async {
    ///     let current = CancellationToken::current().unwrap();
    ///     CancellationToken::child_scope(async {
    ///         assert!(CancellationToken::parent().is_some());
    ///     }).unwrap().await;
    ///     assert!(!current.is_cancelled());
    /// }).await;
    /// # });
    /// ```
    pub fn scope<F: Future>(self, future: F) -> impl Future<Output = F::Output> {
        let parent = Self::current().ok();
        CANCELLATION_SCOPE.scope((self, parent), future)
    }

    /// Create a child of the current token and scope the supplied future to it.
    ///
    /// Captures the parent when called, before the future is polled or spawned.
    /// The child and its descendants are cancelled on completion, panic, or drop.
    /// This signals cancellation; child work must observe its token to stop.
    /// Outside a scope this returns `MissingCancellationScope`; install an
    /// explicit root with `current_or_new().scope(...)` when a fallback is wanted.
    pub fn child_scope<F: Future>(future: F) -> Result<impl Future<Output = F::Output>, TaskErrors> {
        let parent = Self::current()?;
        let token = parent.child_token();
        let guard = token.drop_guard();
        Ok(CANCELLATION_SCOPE.scope((token, Some(parent)), async move {
            let _guard = guard;
            future.await
        }))
    }

    /// Spawn a task in a child scope captured from the current scope.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    pub fn spawn_scoped<F>(future: F) -> Result<JoinHandle<F::Output>, TaskErrors>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        Ok(spawn(Self::child_scope(future)?))
    }

    /// Spawn a browser task in a child scope captured from the current scope.
    #[cfg(all(target_family = "wasm", target_os = "unknown"))]
    pub fn spawn_scoped<F>(future: F) -> Result<JoinHandle<F::Output>, TaskErrors>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        Ok(spawn(Self::child_scope(future)?))
    }

    pub fn new() -> Self {
        Self {
            state: Arc::new(CancelState {
                cancelled: AtomicBool::new(false),
                event: Event::new(),
                children: Mutex::new(Vec::new()),
            }),
            timeout_handle: None,
        }
    }

    pub fn timeout(duration: Duration) -> Self {
        let mut token = CancellationToken::new();
        let child = token.clone();
        let handle = spawn(async move {
            Delay::new(duration).await;
            child.cancel();
        });

        token.timeout_handle = Some(handle);
        token
    }

    /// Returns true if this token or its parent was cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::Acquire)
    }

    pub fn cancel_after(&self, duration: Duration) {
        let token = self.clone();
        spawn(async move {
            Delay::new(duration).await;
            token.cancel();
        });
    }

    /// Cancel this token and recursively cancel all descendant tokens.
    pub fn cancel(&self) {
        if !self.state.cancelled.swap(true, Ordering::AcqRel) {
            self.state.event.notify(usize::MAX);
            let mut stack: Vec<Arc<CancelState>> = Vec::new();
            stack.extend(Self::collect_children(&self.state));
            while let Some(node) = stack.pop() {
                if !node.cancelled.swap(true, Ordering::AcqRel) {
                    node.event.notify(usize::MAX);
                    stack.extend(Self::collect_children(&node));
                }
            }
        }
    }

    fn collect_children(state: &Arc<CancelState>) -> Vec<Arc<CancelState>> {
        let mut children_lock = state.children.lock();
        let mut valid_children = Vec::new();
        children_lock.retain(|weak_ref| {
            if let Some(child) = weak_ref.upgrade() {
                valid_children.push(child);
                true
            } else {
                false
            }
        });
        valid_children
    }

    /// Creates a new child token that is linked to this token.
    pub fn child_token(&self) -> Self {
        let child = CancellationToken::new();

        // Add child to parent's list within a limited scope
        {
            let mut children = self.state.children.lock();
            children.retain(|child| child.strong_count() > 0);
            children.push(Arc::downgrade(&child.state));
        }

        // If parent is already cancelled, cancel the child
        if self.is_cancelled() {
            child.cancel();
        }

        child
    }

    /// Returns a future that resolves when this token is cancelled.
    pub fn cancelled(&self) -> Cancelled {
        Cancelled {
            state: Arc::downgrade(&self.state),
            listener: None,
        }
    }

    pub fn drop_guard(&self) -> DropGuard {
        DropGuard::new(self.clone())
    }
}

pub struct DropGuard {
    token: Option<CancellationToken>,
}

impl DropGuard {
    pub fn new(token: CancellationToken) -> Self {
        Self { token: Some(token) }
    }

    pub fn disarm(&mut self) {
        self.token = None;
    }
}

impl Drop for DropGuard {
    fn drop(&mut self) {
        if let Some(token) = &self.token {
            token.cancel();
        }
    }
}

pub struct Cancelled {
    state: Weak<CancelState>,
    listener: Option<EventListener>,
}

impl Future for Cancelled {
    type Output = ();

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();
        let Some(state) = this.state.upgrade() else {
            // Parent is dropped, consider it cancelled
            return std::task::Poll::Ready(());
        };
        if state.cancelled.load(Ordering::Acquire) {
            return std::task::Poll::Ready(());
        }
        let listener = this.listener.get_or_insert_with(|| state.event.listen());
        if state.cancelled.load(Ordering::Acquire) {
            std::task::Poll::Ready(())
        } else {
            std::pin::Pin::new(listener).poll(cx).map(|_| ())
        }
    }
}

use futures_timer::Delay;
use futures_util::{select, FutureExt};
use n0_future::pin;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TaskErrors {
    #[error("No cancellation scope is active")]
    MissingCancellationScope,
    #[error("Task cancelled")]
    Cancelled,
}

pub trait FutureExtension: Future + Sized {
    fn with_cancel(self, cancellation: &CancellationToken) -> impl Future<Output = Result<Self::Output, TaskErrors>>;
}

impl<T: Future> FutureExtension for T {
    async fn with_cancel(self, cancellation: &CancellationToken) -> Result<Self::Output, TaskErrors> {
        if cancellation.is_cancelled() {
            return Err(TaskErrors::Cancelled);
        }

        let self_ = self.fuse();
        pin!(self_);
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

#[cfg(test)]
mod tests {
    use super::*;
    use futures::task::{waker_ref, ArcWake};
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn missing_scope_and_fallback_tokens_are_explicit() {
        assert!(matches!(
            CancellationToken::current(),
            Err(TaskErrors::MissingCancellationScope)
        ));
        assert!(CancellationToken::parent().is_none());
        assert!(matches!(
            CancellationToken::child_scope(async {}),
            Err(TaskErrors::MissingCancellationScope)
        ));
        let first = CancellationToken::current_or_new();
        let second = CancellationToken::current_or_new();
        first.cancel();
        assert!(!second.is_cancelled());
        assert!(CancellationToken::current().is_err());
    }

    #[test]
    fn current_or_new_shares_the_scoped_token() {
        let root = CancellationToken::new();
        futures::executor::block_on(root.clone().scope(async {
            CancellationToken::current_or_new().cancel();
            assert!(CancellationToken::current().unwrap().is_cancelled());
            assert!(CancellationToken::current_or_new().is_cancelled());
            assert!(CancellationToken::parent().is_none());
        }));
        assert!(root.is_cancelled());
        assert!(CancellationToken::current().is_err());
    }

    #[test]
    fn nested_scopes_restore_the_enclosing_token() {
        let root = CancellationToken::new();
        let nested = CancellationToken::new();
        futures::executor::block_on(root.clone().scope(async {
            nested
                .clone()
                .scope(async {
                    assert!(Arc::ptr_eq(&CancellationToken::current().unwrap().state, &nested.state));
                    assert!(Arc::ptr_eq(&CancellationToken::parent().unwrap().state, &root.state));
                })
                .await;
            assert!(Arc::ptr_eq(&CancellationToken::current().unwrap().state, &root.state));
            assert!(CancellationToken::parent().is_none());
        }));
        assert!(CancellationToken::current().is_err());
        assert!(!root.is_cancelled());
        assert!(!nested.is_cancelled());
    }

    #[test]
    fn child_scope_completion_cancels_descendants_but_not_siblings_or_parent() {
        let root = CancellationToken::new();
        let sibling = root.child_token();
        let (child, descendant) = futures::executor::block_on(root.clone().scope(async {
            CancellationToken::child_scope(async {
                let child = CancellationToken::current().unwrap();
                assert!(Arc::ptr_eq(&CancellationToken::parent().unwrap().state, &root.state));
                let descendant = child.child_token();
                (child, descendant)
            })
            .unwrap()
            .await
        }));
        assert!(child.is_cancelled());
        assert!(descendant.is_cancelled());
        assert!(!root.is_cancelled());
        assert!(!sibling.is_cancelled());
    }

    #[test]
    fn parent_cancellation_reaches_the_scoped_child() {
        let root = CancellationToken::new();
        futures::executor::block_on(root.clone().scope(async {
            CancellationToken::child_scope(async {
                let child = CancellationToken::current().unwrap();
                root.cancel();
                child.cancelled().await;
                assert!(child.is_cancelled());
            })
            .unwrap()
            .await;
        }));
    }

    #[test]
    fn dropping_an_unpolled_child_scope_cancels_its_token() {
        let root = CancellationToken::new();
        futures::executor::block_on(root.clone().scope(async {
            let child = CancellationToken::child_scope(futures::future::pending::<()>()).unwrap();
            let child_state = root.state.children.lock()[0].upgrade().unwrap();
            drop(child);
            assert!(child_state.cancelled.load(Ordering::Acquire));
            assert!(Arc::ptr_eq(&CancellationToken::current().unwrap().state, &root.state));
        }));
        assert!(!root.is_cancelled());
    }

    #[test]
    fn dropping_a_suspended_child_scope_cancels_and_restores_context() {
        let root = CancellationToken::new();
        futures::executor::block_on(root.clone().scope(async {
            let observed = std::cell::RefCell::new(None);
            let mut scoped = Box::pin(
                CancellationToken::child_scope(async {
                    *observed.borrow_mut() = Some(CancellationToken::current().unwrap());
                    futures::future::pending::<()>().await;
                })
                .unwrap(),
            );
            assert!(futures::poll!(&mut scoped).is_pending());
            assert!(Arc::ptr_eq(&CancellationToken::current().unwrap().state, &root.state));
            drop(scoped);
            assert!(observed.borrow().as_ref().unwrap().is_cancelled());
        }));
        assert!(CancellationToken::current().is_err());
        assert!(!root.is_cancelled());
    }

    #[test]
    fn panicking_child_scope_cancels_and_restores_context() {
        let root = CancellationToken::new();
        futures::executor::block_on(root.clone().scope(async {
            let observed = std::cell::RefCell::new(None);
            let scoped = CancellationToken::child_scope(async {
                *observed.borrow_mut() = Some(CancellationToken::current().unwrap());
                panic!("scope panic");
            })
            .unwrap();
            assert!(std::panic::AssertUnwindSafe(scoped).catch_unwind().await.is_err());
            assert!(observed.borrow().as_ref().unwrap().is_cancelled());
            assert!(Arc::ptr_eq(&CancellationToken::current().unwrap().state, &root.state));
        }));
        assert!(!root.is_cancelled());
    }

    #[test]
    fn interleaved_scopes_keep_separate_tokens_across_awaits() {
        let first = CancellationToken::new();
        let second = CancellationToken::new();
        futures::executor::block_on(async {
            let one = first.clone().scope(async {
                let mut pending = true;
                futures::future::poll_fn(|cx| {
                    assert!(Arc::ptr_eq(&CancellationToken::current().unwrap().state, &first.state));
                    if std::mem::take(&mut pending) {
                        cx.waker().wake_by_ref();
                        std::task::Poll::Pending
                    } else {
                        std::task::Poll::Ready(())
                    }
                })
                .await;
                CancellationToken::current().unwrap().cancel();
            });
            let two = second.clone().scope(async {
                let mut pending = true;
                futures::future::poll_fn(|cx| {
                    assert!(Arc::ptr_eq(&CancellationToken::current().unwrap().state, &second.state));
                    if std::mem::take(&mut pending) {
                        cx.waker().wake_by_ref();
                        std::task::Poll::Pending
                    } else {
                        std::task::Poll::Ready(())
                    }
                })
                .await;
                assert!(!CancellationToken::current().unwrap().is_cancelled());
            });
            futures::join!(one, two);
        });
        assert!(first.is_cancelled());
        assert!(!second.is_cancelled());
        assert!(CancellationToken::current().is_err());
    }

    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    #[test]
    fn spawned_tasks_inherit_a_child_scope_and_abort_cancels_it() {
        let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).build().unwrap();
        runtime.block_on(async {
            assert!(matches!(
                CancellationToken::spawn_scoped(async {}),
                Err(TaskErrors::MissingCancellationScope)
            ));
            let root = CancellationToken::new();
            root.clone()
                .scope(async {
                    let expected_parent = root.clone();
                    let completed = CancellationToken::spawn_scoped(async move {
                        tokio::task::yield_now().await;
                        assert!(Arc::ptr_eq(&CancellationToken::parent().unwrap().state, &expected_parent.state));
                        CancellationToken::current().unwrap()
                    })
                    .unwrap()
                    .await
                    .unwrap();
                    assert!(completed.is_cancelled());
                    assert!(tokio::spawn(async { CancellationToken::current().is_err() }).await.unwrap());

                    let (sender, receiver) = tokio::sync::oneshot::channel();
                    let running = CancellationToken::spawn_scoped(async move {
                        sender.send(CancellationToken::current().unwrap()).unwrap();
                        futures::future::pending::<()>().await;
                    })
                    .unwrap();
                    let child = receiver.await.unwrap();
                    running.abort();
                    assert!(running.await.unwrap_err().is_cancelled());
                    assert!(child.is_cancelled());
                    assert!(!root.is_cancelled());
                })
                .await;
            assert!(CancellationToken::current().is_err());
        });
    }

    struct WakeCounter(AtomicUsize);

    impl ArcWake for WakeCounter {
        fn wake_by_ref(arc_self: &Arc<Self>) {
            arc_self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn test_cancellation_token_basic() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());

        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_child_token() {
        let parent = CancellationToken::new();
        let child = parent.child_token();

        assert!(!parent.is_cancelled());
        assert!(!child.is_cancelled());

        parent.cancel();
        assert!(parent.is_cancelled());
        assert!(child.is_cancelled());
    }

    #[test]
    fn cancellation_wakes_every_waiter() {
        let token = CancellationToken::new();
        let mut first = Box::pin(token.cancelled());
        let mut second = Box::pin(token.clone().cancelled());
        let first_counter = Arc::new(WakeCounter(AtomicUsize::new(0)));
        let second_counter = Arc::new(WakeCounter(AtomicUsize::new(0)));
        let first_waker = waker_ref(&first_counter);
        let second_waker = waker_ref(&second_counter);
        let mut first_context = std::task::Context::from_waker(&first_waker);
        let mut second_context = std::task::Context::from_waker(&second_waker);

        assert!(first.as_mut().poll(&mut first_context).is_pending());
        assert!(second.as_mut().poll(&mut second_context).is_pending());
        token.cancel();

        assert_eq!(first_counter.0.load(Ordering::Relaxed), 1);
        assert_eq!(second_counter.0.load(Ordering::Relaxed), 1);
        assert!(first.as_mut().poll(&mut first_context).is_ready());
        assert!(second.as_mut().poll(&mut second_context).is_ready());
    }
}
