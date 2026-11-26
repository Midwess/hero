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
//! - [`cancel`](CancellationToken::cancel) marks the token and its descendants as cancelled before returning and
//!   signals their waiters. Work stops only where it
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
//! - [`timeout`](CancellationToken::timeout), [`child_timeout`](CancellationToken::child_timeout), and
//!   [`cancel_after`](CancellationToken::cancel_after) measure deadlines from the call site. Dropping the handle
//!   returned by either timeout constructor aborts its timer. Clones share the signal without owning the timer.
//! - Equality says nothing: two tokens always compare equal, so containing types can derive it. Serialization carries
//!   nothing: a deserialized token is a fresh, uncancelled root.

use event_listener::{listener, Event, EventListener};
use n0_future::task::{spawn, JoinHandle};
#[cfg(target_family = "wasm")]
use n0_future::time::Instant;
use parking_lot::Mutex;
use std::fmt::Debug;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, LazyLock, Weak};
#[cfg(not(target_family = "wasm"))]
use std::time::Instant;

tokio::task_local! {
    static CANCELLATION_SCOPE: (CancellationToken, Option<CancellationToken>);
}

#[derive(Debug, Default)]
struct CancelState {
    status: AtomicU8,
    event: Event,
    children: Mutex<Children>,
}

const MARKED: u8 = 1;
const COMPLETE: u8 = 2;
// Set before registering a waiter so notifications can skip unused events.
const LISTENING: u8 = 4;
static CANCELLED_STATE: LazyLock<Arc<CancelState>> = LazyLock::new(|| {
    let state = CancelState::default();
    state.status.store(MARKED | COMPLETE, Ordering::Relaxed);
    Arc::new(state)
});

impl CancelState {
    fn mark(&self, pending: &mut Vec<Arc<Self>>) -> bool {
        let children = self.children.lock();
        let status = self.status.load(Ordering::Acquire);
        if status & COMPLETE != 0 {
            return false;
        }
        if status & MARKED == 0 {
            self.status.fetch_or(MARKED, Ordering::SeqCst);
        }
        pending.extend(children.entries.iter().filter_map(Weak::upgrade));
        true
    }

    fn finish(&self) {
        let status = self.status.fetch_or(COMPLETE, Ordering::AcqRel);
        if status & COMPLETE == 0 {
            let entries = std::mem::take(&mut self.children.lock().entries);
            drop(entries);
            if status & LISTENING != 0 {
                self.event.notify(usize::MAX);
            }
        }
    }

    #[inline]
    fn listener_event(&self) -> &Event {
        if self.status.load(Ordering::Relaxed) & LISTENING == 0 {
            self.status.fetch_or(LISTENING, Ordering::SeqCst);
        }
        &self.event
    }

    #[inline]
    fn cancelled_after_listen(&self) -> bool {
        self.status.load(Ordering::SeqCst) & MARKED != 0
    }
}

impl Drop for CancelState {
    fn drop(&mut self) {
        if *self.status.get_mut() & (LISTENING | COMPLETE) == LISTENING {
            self.event.notify(usize::MAX);
        }
    }
}

#[derive(Debug, Default)]
struct Children {
    entries: Vec<Weak<CancelState>>,
    until_cleanup: usize,
}

#[derive(Serialize, Deserialize)]
/// A clonable cancellation signal that can form parent-child trees.
///
/// Clones observe the same state. Use [`CancellationToken::child_token`] when
/// cancellation should flow from one unit of work into another without flowing
/// back to the parent.
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

    /// Checks the active scope without cloning its token. Returns false outside a scope.
    ///
    /// In tight loops, borrow a token outside the loop to skip repeated scope lookups.
    #[inline(always)]
    pub fn is_current_cancelled() -> bool {
        CANCELLATION_SCOPE.try_with(|(token, _)| token.is_cancelled()).unwrap_or(false)
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

    /// Creates an independent, initially active root token.
    pub fn new() -> Self {
        Self {
            state: Arc::new(CancelState::default()),
            timeout_handle: None,
        }
    }

    /// Creates a token that cancels itself after a delay.
    ///
    /// The returned handle owns the timer. Dropping that handle cancels the
    /// timer; clones continue to share cancellation state but do not own it.
    pub fn timeout(duration: Duration) -> Self {
        Self::new().with_owned_timer(duration)
    }

    /// Creates a child with a deadline measured from this call.
    ///
    /// Parent cancellation propagates to the child. Dropping the returned handle
    /// stops its timer, and clones share the child's signal without owning the timer.
    pub fn child_timeout(&self, duration: Duration) -> Self {
        self.child_token().with_owned_timer(duration)
    }

    /// Checks the cancellation flag without synchronizing unrelated memory.
    #[inline(always)]
    pub fn is_cancelled(&self) -> bool {
        self.state.status.load(Ordering::Relaxed) & MARKED != 0
    }

    /// Schedules cancellation after a delay without tying the timer to a handle.
    ///
    /// Calling this more than once is harmless; the earliest timer to fire wins.
    /// Timers release their resources promptly when the token is cancelled.
    pub fn cancel_after(&self, duration: Duration) {
        self.start_timer(duration);
    }

    fn with_owned_timer(mut self, duration: Duration) -> Self {
        self.timeout_handle = self.start_timer(duration);
        self
    }

    fn start_timer(&self, duration: Duration) -> Option<JoinHandle<()>> {
        if self.is_cancelled() {
            return None;
        }
        let started = Instant::now();
        let token = self.clone();
        Some(spawn(async move {
            if token.is_cancelled() {
                return;
            }
            let remaining = duration.saturating_sub(started.elapsed());
            if remaining.is_zero() || Delay::new(remaining).with_cancel(&token).await.is_ok() {
                token.cancel();
            }
        }))
    }

    /// Marks this token and all attached descendants as cancelled before returning.
    pub fn cancel(&self) {
        if self.state.status.load(Ordering::Acquire) & COMPLETE != 0 {
            return;
        }
        let mut pending = Vec::new();
        if !self.state.mark(&mut pending) {
            return;
        }
        let mut next = 0;
        let mut discovered = Vec::new();
        while next < pending.len() {
            for node in &pending[next..] {
                node.mark(&mut discovered);
            }
            next = pending.len();
            pending.append(&mut discovered);
        }

        // Keep edges visible until the subtree is marked. Wakers can run arbitrary code.
        for node in pending.into_iter().rev() {
            node.finish();
        }
        self.state.finish();
    }

    /// Creates a new child token that is linked to this token.
    pub fn child_token(&self) -> Self {
        if self.is_cancelled() {
            return Self::already_cancelled();
        }
        let child = CancellationToken::new();
        let mut children = self.state.children.lock();
        if self.is_cancelled() {
            return Self::already_cancelled();
        }
        if children.until_cleanup == 0 {
            children.entries.retain(|child| child.strong_count() > 0);
            children.until_cleanup = children.entries.len().max(64);
        }
        children.until_cleanup -= 1;
        children.entries.push(Arc::downgrade(&child.state));
        child
    }

    fn already_cancelled() -> Self {
        Self {
            state: Arc::clone(&CANCELLED_STATE),
            timeout_handle: None,
        }
    }

    /// Returns a future that resolves when this token is cancelled.
    pub fn cancelled(&self) -> Cancelled {
        Cancelled {
            state: Arc::downgrade(&self.state),
            listener: None,
        }
    }

    /// Creates a guard that cancels this token when the guard is dropped.
    ///
    /// Use [`DropGuard::disarm`] to transfer responsibility elsewhere.
    pub fn drop_guard(&self) -> DropGuard {
        DropGuard::new(self.clone())
    }
}

/// Cancels its token on drop unless explicitly disarmed.
pub struct DropGuard {
    token: Option<CancellationToken>,
}

impl DropGuard {
    /// Begins guarding a token.
    pub fn new(token: CancellationToken) -> Self {
        Self { token: Some(token) }
    }

    /// Prevents cancellation when this guard is dropped.
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

/// Future returned by [`CancellationToken::cancelled`].
///
/// It also resolves when the last strong handle to the token disappears, so a
/// waiter cannot remain suspended after its cancellation source is gone.
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
        if state.status.load(Ordering::Relaxed) & MARKED != 0 {
            return std::task::Poll::Ready(());
        }
        let listener = this.listener.get_or_insert_with(|| state.listener_event().listen());
        if state.cancelled_after_listen() {
            std::task::Poll::Ready(())
        } else {
            std::pin::Pin::new(listener).poll(cx).map(|_| ())
        }
    }
}

use futures_timer::Delay;
use serde::{Deserialize, Serialize};
use std::future::{poll_fn, Future};
use std::pin::{pin, Pin};
use std::task::Poll;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
/// Failures produced by scoped or cancellation-aware task helpers.
pub enum TaskErrors {
    /// An operation requiring [`CancellationToken::current`] ran outside a scope.
    #[error("No cancellation scope is active")]
    MissingCancellationScope,
    /// Cancellation won the race against the wrapped future.
    #[error("Task cancelled")]
    Cancelled,
}

/// Adds cancellation-aware completion to every future.
pub trait FutureExtension: Future + Sized {
    /// Races this future against a token without spawning either branch.
    ///
    /// If the token is already cancelled, the future is never polled. When
    /// cancellation wins later, the future is dropped in place.
    fn with_cancel(self, cancellation: &CancellationToken) -> impl Future<Output = Result<Self::Output, TaskErrors>>;
}

impl<T: Future> FutureExtension for T {
    #[inline]
    async fn with_cancel(self, cancellation: &CancellationToken) -> Result<Self::Output, TaskErrors> {
        let mut future = pin!(self);
        match poll_fn(|cx| {
            if cancellation.is_cancelled() {
                Poll::Ready(Err(TaskErrors::Cancelled))
            } else {
                Poll::Ready(Ok(future.as_mut().poll(cx)))
            }
        })
        .await?
        {
            Poll::Ready(output) => return Ok(output),
            Poll::Pending => {}
        }

        listener!(cancellation.state.listener_event() => cancelled);
        if cancellation.state.cancelled_after_listen() {
            return Err(TaskErrors::Cancelled);
        }
        let mut first = true;
        poll_fn(|cx| {
            if cancellation.is_cancelled() || Pin::new(&mut cancelled).poll(cx).is_ready() {
                return Poll::Ready(Err(TaskErrors::Cancelled));
            }
            if std::mem::take(&mut first) {
                return Poll::Pending;
            }
            future.as_mut().poll(cx).map(Ok)
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::task::{waker_ref, ArcWake};
    use futures::FutureExt;
    use std::sync::atomic::AtomicUsize;

    #[cfg(not(target_family = "wasm"))]
    #[tokio::test]
    async fn cancelled_timers_release_their_tokens_promptly() {
        for token in [
            CancellationToken::timeout(Duration::from_secs(60)),
            CancellationToken::new(),
        ] {
            token.cancel_after(Duration::from_secs(60));
            tokio::task::yield_now().await;
            token.cancel();
            for _ in 0..1000 {
                if Arc::strong_count(&token.state) == 1 {
                    break;
                }
                tokio::task::yield_now().await;
            }
            assert_eq!(Arc::strong_count(&token.state), 1);
        }
        let token = CancellationToken::new();
        token.cancel();
        token.cancel_after(Duration::from_secs(60));
        assert_eq!(Arc::strong_count(&token.state), 1);
    }

    #[cfg(not(target_family = "wasm"))]
    #[tokio::test]
    async fn parent_cancellation_releases_a_large_timer_backlog() {
        let root = CancellationToken::new();
        let children: Vec<_> = (0..10000)
            .map(|_| {
                let child = root.child_token();
                child.cancel_after(Duration::from_secs(60));
                child
            })
            .collect();
        tokio::task::yield_now().await;
        root.cancel();
        for _ in 0..1000 {
            if children.iter().all(|child| Arc::strong_count(&child.state) == 1) {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("cancelled timer tasks retained child states");
    }

    #[cfg(not(target_family = "wasm"))]
    #[tokio::test]
    async fn timer_ownership_and_detached_deadlines_are_preserved() {
        let token = CancellationToken::timeout(Duration::from_millis(10));
        let observer = token.clone();
        drop(token);
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(!observer.is_cancelled());
        let token = CancellationToken::timeout(Duration::from_millis(10));
        drop(token.clone());
        token.cancelled().await;
        assert!(token.is_cancelled());
        let token = CancellationToken::new();
        let observer = token.clone();
        token.cancel_after(Duration::from_millis(10));
        token.cancel_after(Duration::from_secs(60));
        drop(token);
        observer.cancelled().await;
        assert!(observer.is_cancelled());
    }

    #[test]
    fn cancelled_parent_and_dead_child_cleanup_remain_bounded() {
        let root = CancellationToken::new();
        for _ in 0..10000 {
            drop(root.child_token());
        }
        assert!(root.state.children.lock().entries.len() <= 64);
        root.cancel();
        assert!(root.child_token().is_cancelled());
        assert!(root.state.children.lock().entries.is_empty());
    }

    #[test]
    fn dropping_parent_detaches_children_and_resolves_waiters() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        let mut waiter = Box::pin(parent.cancelled());
        let counter = Arc::new(WakeCounter(AtomicUsize::new(0)));
        let waker = waker_ref(&counter);
        let mut cx = std::task::Context::from_waker(&waker);
        assert!(waiter.as_mut().poll(&mut cx).is_pending());
        drop(parent);
        assert_eq!(counter.0.load(Ordering::Relaxed), 1);
        assert!(waiter.as_mut().poll(&mut cx).is_ready());
        assert!(!child.is_cancelled());
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn child_registration_racing_cancellation_cannot_escape() {
        for _ in 0..100 {
            let root = CancellationToken::new();
            std::thread::scope(|scope| {
                let cancellation = scope.spawn(|| root.cancel());
                let children: Vec<_> = (0..100).map(|_| root.child_token()).collect();
                cancellation.join().unwrap();
                root.cancel();
                assert!(children.iter().all(CancellationToken::is_cancelled));
            });
        }
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn waiter_registration_racing_cancellation_cannot_lose_a_wakeup() {
        for _ in 0..100 {
            let token = CancellationToken::new();
            let counter = Arc::new(WakeCounter(AtomicUsize::new(0)));
            let waker = waker_ref(&counter);
            let mut cx = std::task::Context::from_waker(&waker);
            let mut waiter = Box::pin(token.cancelled());
            std::thread::scope(|scope| {
                let cancellation = scope.spawn(|| token.cancel());
                let pending = waiter.as_mut().poll(&mut cx).is_pending();
                cancellation.join().unwrap();
                if pending {
                    assert!(counter.0.load(Ordering::Relaxed) >= 1);
                }
                assert!(waiter.as_mut().poll(&mut cx).is_ready());
            });
        }
    }

    #[test]
    fn deep_cancellation_does_not_recurse() {
        let root = CancellationToken::new();
        let mut nodes = vec![root.clone()];
        for _ in 0..10000 {
            nodes.push(nodes.last().unwrap().child_token());
        }
        root.cancel();
        assert!(nodes.iter().all(CancellationToken::is_cancelled));
    }

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
            let child_state = root.state.children.lock().entries[0].upgrade().unwrap();
            drop(child);
            assert_ne!(child_state.status.load(Ordering::Acquire) & MARKED, 0);
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
