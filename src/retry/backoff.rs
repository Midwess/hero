use std::fmt::Display;
use std::future::Future;
use std::time::Duration;

use log::warn;
use rand::Rng;
use tokio::time::sleep;

pub async fn with_capped_backoff<F, Fut, T, E>(initial: Duration, max: Duration, label: &str, mut op: F) -> T
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: Display,
{
    let mut delay = initial;
    loop {
        match op().await {
            Ok(value) => return value,
            Err(err) => {
                let jitter = rand::rng().random_range(0.75..=1.25);
                let sleep_for = delay.mul_f64(jitter).min(max);
                warn!(
                    target: "devlog-sdk-retry",
                    "{label} attempt failed: {err}; retrying in {}ms",
                    sleep_for.as_millis()
                );
                sleep(sleep_for).await;
                delay = (delay * 2).min(max);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn returns_first_ok_after_retries() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_for_op = attempts.clone();

        let value = with_capped_backoff(Duration::from_millis(10), Duration::from_millis(80), "test", || {
            let attempts = attempts_for_op.clone();
            async move {
                let n = attempts.fetch_add(1, Ordering::SeqCst);
                if n < 2 {
                    Err::<i32, &'static str>("boom")
                } else {
                    Ok(7)
                }
            }
        })
        .await;

        assert_eq!(value, 7);
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn delay_caps_at_max() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_for_op = attempts.clone();

        let value = with_capped_backoff(Duration::from_millis(10), Duration::from_millis(20), "cap-test", || {
            let attempts = attempts_for_op.clone();
            async move {
                let n = attempts.fetch_add(1, Ordering::SeqCst);
                if n < 5 {
                    Err::<&'static str, &'static str>("again")
                } else {
                    Ok("done")
                }
            }
        })
        .await;

        assert_eq!(value, "done");
        assert_eq!(attempts.load(Ordering::SeqCst), 6);
    }
}
