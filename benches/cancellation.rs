use hero::cancellation::CancellationToken;
use std::time::Instant;
fn main() {
    for n in [1000, 10000, 50000] {
        let root = CancellationToken::new();
        let start = Instant::now();
        let children: Vec<_> = (0..n).map(|_| root.child_token()).collect();
        let registration = start.elapsed();
        let start = Instant::now();
        root.cancel();
        println!("children={n} register={registration:?} cancel={:?}", start.elapsed());
        std::hint::black_box(children);
    }
    let token = CancellationToken::new();
    token.cancel();
    let start = Instant::now();
    for _ in 0..10000000 {
        std::hint::black_box(&token).cancel();
    }
    println!("repeat_cancel={:?}", start.elapsed());
}
