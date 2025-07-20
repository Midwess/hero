use log::info;
use pretty_env_logger::formatted_timed_builder;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::{self};

pub fn setup() {
    #[cfg(feature = "wasm-logger")]
    {
        let _ = console_log::init();
        console_error_panic_hook::set_once();
    };

    #[cfg(not(feature = "wasm-logger"))]
    {
        let mut log_builder = formatted_timed_builder();
        if let Ok(filter_env) = std::env::var("RUST_LOG") {
            log_builder.parse_filters(&filter_env);
        } else {
            log_builder.filter(None, log::LevelFilter::Info);
        }

        log_builder.init();
    };
}

pub struct ThrottleLogger {
    messages: Arc<Mutex<HashMap<String, usize>>>,
    join_handle: JoinHandle<()>
}

impl ThrottleLogger {
    pub fn new(namespace: String, delay: Duration) -> Self {
        let messages = Arc::new(Mutex::new(HashMap::new()));
        let messages_clone = messages.clone();
        let ns = namespace.clone();

        let join_handle = tokio::spawn(async move {
            let mut interval = time::interval(delay);
            interval.tick().await; // initial delay

            loop {
                interval.tick().await;

                let mut msgs = messages_clone.lock().await;
                for (content, count) in msgs.drain() {
                    info!(
                        target: ns.as_str(),
                        "{content} happens {count} times in the last {delay:?}"
                    );
                }
            }
        });

        Self { messages, join_handle }
    }

    pub async fn log(&self, content: String) {
        let mut msgs = self.messages.lock().await;
        *msgs.entry(content).or_insert(0) += 1;
    }
}

impl Drop for ThrottleLogger {
    fn drop(&mut self) {
        self.join_handle.abort();
    }
}
