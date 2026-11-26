use hero::cancellation::{CancellationToken, FutureExtension};
use std::future::{pending, ready, Future};
use std::hint::black_box;
use std::pin::pin;
use std::task::{Context, Waker};
use std::time::Instant;

fn measure(label: &str, mut iteration: impl FnMut()) {
    const COUNT: u32 = 200_000;
    let mut samples = [0.0; 7];
    for sample in &mut samples {
        let started = Instant::now();
        for _ in 0..COUNT {
            iteration();
        }
        *sample = started.elapsed().as_secs_f64() * 1e9 / f64::from(COUNT);
    }
    samples.sort_by(f64::total_cmp);
    println!("{label}: {:.2} ns/op", samples[3]);
}

fn main() {
    let token = CancellationToken::new();
    let mut cx = Context::from_waker(Waker::noop());
    measure("borrowed cancellation check", || {
        black_box(black_box(&token).is_cancelled());
    });
    futures::executor::block_on(token.clone().scope(async {
        measure("scope check without clone", || {
            black_box(CancellationToken::is_current_cancelled());
        });
        measure("scope check with clone", || {
            black_box(CancellationToken::current().unwrap().is_cancelled());
        });
    }));
    measure("ready with_cancel", || {
        let _ = black_box(pin!(ready(black_box(42)).with_cancel(black_box(&token))).poll(&mut cx));
    });
    let mut waiting = pin!(pending::<()>().with_cancel(&token));
    assert!(waiting.as_mut().poll(&mut cx).is_pending());
    measure("first pending poll and drop", || {
        let _ = black_box(pin!(pending::<()>().with_cancel(black_box(&token))).poll(&mut cx));
    });
    measure("repeat pending poll", || {
        let _ = black_box(waiting.as_mut().poll(&mut cx));
    });
    measure("new root and drop", || {
        black_box(CancellationToken::new());
    });
    measure("active child and drop", || {
        black_box(black_box(&token).child_token());
    });
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    measure("cancelled child and drop", || {
        black_box(black_box(&cancelled).child_token());
    });

    for n in [1000, 10000, 50000] {
        let root = CancellationToken::new();
        let start = Instant::now();
        let children: Vec<_> = (0..n).map(|_| root.child_token()).collect();
        let registration = start.elapsed();
        let start = Instant::now();
        root.cancel();
        println!("children={n} register={registration:?} cancel={:?}", start.elapsed());
        black_box(children);
    }
    let token = CancellationToken::new();
    token.cancel();
    let start = Instant::now();
    for _ in 0..10000000 {
        black_box(&token).cancel();
    }
    println!("repeat_cancel={:?}", start.elapsed());
}
