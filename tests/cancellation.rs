#![cfg(all(feature = "cancellation", not(target_family = "wasm")))]

use hero::cancellation::{CancellationToken, FutureExtension, TaskErrors};
use std::future::{poll_fn, Future};
use std::pin::pin;
use std::sync::{Arc, Barrier};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

#[test]
fn cancellation_during_the_first_poll_is_observed_before_parking() {
    let token = CancellationToken::new();
    let operation = poll_fn(|_| {
        token.cancel();
        Poll::<()>::Pending
    });
    let mut wrapped = pin!(operation.with_cancel(&token));
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(
        wrapped.as_mut().poll(&mut context),
        Poll::Ready(Err(TaskErrors::Cancelled))
    ));
}

#[test]
fn wrapping_a_send_future_preserves_send_for_local_outputs() {
    fn assert_send<T: Send>(_: &T) {}

    let token = CancellationToken::new();
    let wrapped = async { std::rc::Rc::new(7) }.with_cancel(&token);
    assert_send(&wrapped);
    assert_eq!(*futures::executor::block_on(wrapped).unwrap(), 7);
}

#[test]
fn active_scope_checks_restore_after_nested_cancellation() {
    let parent = CancellationToken::new();
    let child = parent.child_token();
    assert!(!CancellationToken::is_current_cancelled());
    futures::executor::block_on(parent.scope(async {
        assert!(!CancellationToken::is_current_cancelled());
        child
            .clone()
            .scope(async {
                child.cancel();
                assert!(CancellationToken::is_current_cancelled());
            })
            .await;
        assert!(!CancellationToken::is_current_cancelled());
    }));
}

#[tokio::test]
async fn deadlines_include_scheduler_delay_and_owned_children_abort_on_drop() {
    let parent = CancellationToken::new();
    let expired = parent.child_timeout(Duration::from_millis(20));
    let owner = parent.child_timeout(Duration::from_millis(20));
    let observer = owner.clone();
    drop(owner);
    std::thread::sleep(Duration::from_millis(40));
    tokio::task::yield_now().await;
    assert!(expired.is_cancelled());
    assert!(!observer.is_cancelled());
    parent.cancel();
    assert!(observer.is_cancelled());
}

struct BlockingWake {
    started: Barrier,
    resume: Barrier,
}

impl Wake for BlockingWake {
    fn wake(self: Arc<Self>) {
        self.started.wait();
        self.resume.wait();
    }
}

#[test]
fn parent_cancellation_marks_descendants_during_concurrent_child_cancellation() {
    let parent = CancellationToken::new();
    let child = parent.child_token();
    let grandchild = child.child_token();
    let wake = Arc::new(BlockingWake {
        started: Barrier::new(2),
        resume: Barrier::new(2),
    });
    let waker = Waker::from(Arc::clone(&wake));
    let mut context = Context::from_waker(&waker);
    let mut waiter = pin!(child.cancelled());
    assert!(waiter.as_mut().poll(&mut context).is_pending());

    let cancelling = std::thread::spawn(move || child.cancel());
    wake.started.wait();
    parent.cancel();
    let descendant_was_cancelled = grandchild.is_cancelled();
    wake.resume.wait();
    cancelling.join().unwrap();
    assert!(descendant_was_cancelled);
}
