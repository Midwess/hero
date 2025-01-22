#[macro_export]
macro_rules! retry {
    (
        retries = $max_retries:expr,
        delay = $delay:expr,
        $($on_error:expr,)?
        $block:block
    ) => {{
        use std::time::Duration;
        use tokio::time::sleep;
        use log::warn;

        let mut attempt = 0;
        loop {
            attempt += 1;

            let result = async {
                $block
            }.await;

            match result {
                Ok(val) => break Ok(val), // Exit early on success
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

                    sleep($delay).await;
                }
            }
        }
    }};
}
