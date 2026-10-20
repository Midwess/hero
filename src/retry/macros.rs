/// Retries an async block with a fixed delay.
///
/// The block must evaluate to a `Result`. `retries` counts the initial attempt,
/// so `retries = 1` never sleeps. An optional callback runs after each failed
/// attempt, including the final one, and receives a shared reference to the
/// error.
///
/// # Example
///
/// ```
/// # async fn example() -> Result<(), &'static str> {
/// let value = hero::retry!(
///     retries = 3,
///     delay = std::time::Duration::from_millis(10),
///     |_| {},
///     { Ok::<_, &'static str>(42) }
/// )?;
/// assert_eq!(value, 42);
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! retry {
    (
        retries = $max_retries:expr,
        delay = $delay:expr,
        $($on_error:expr,)?
        $block:block
    ) => {{
        use std::time::Duration;
        use log::warn;

        let mut attempt = 0;
        loop {
            attempt += 1;

            let result = async {
                $block
            }.await;

            match result {
                Ok(val) => break Ok(val),
                Err(e) => {
                    let should_retry = {
                        $(
                            $on_error(&e);
                        )?

                        true
                    };

                    if attempt >= $max_retries || !should_retry {
                        break Err(e);
                    }

                    warn!(
                        "Attempt {}/{} failed with error: {}. Retrying after {:?}...",
                        attempt,
                        $max_retries,
                        e,
                        $delay
                    );

                    futures_timer::Delay::new($delay).await;
                }
            }
        }
    }};
}
