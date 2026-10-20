//! Logging defaults for native processes and browser applications.
//!
//! Native setup honors `RUST_LOG`. Setting `FILE_LOG` enables an uncolored tee
//! to `FILE_LOG_PATH`, or to `logs/sandbox-<SANDBOX_ID>.log` when no path is
//! supplied. Browser setup writes informational records to the console and can
//! be toggled through the global `__midwess_log` value.

use futures_util::lock::Mutex;
use log::info;
use n0_future::task::{spawn, JoinHandle};
use n0_future::time;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Installs Hero's process logger if another logger has not already won.
///
/// Repeated calls are harmless. Native builds suppress known noisy targets and
/// browser builds also install the panic hook.
pub fn setup() {
    #[cfg(target_arch = "wasm32")]
    {
        use js_sys::Reflect;
        use log::{Level, LevelFilter, Log, Metadata, Record};
        use wasm_bindgen::JsValue;

        fn log_flag_enabled() -> bool {
            let global = js_sys::global();
            match Reflect::get(&global, &JsValue::from_str("__midwess_log")) {
                Ok(v) if v.is_undefined() => cfg!(debug_assertions),
                Ok(v) => v.is_truthy(),
                Err(_) => cfg!(debug_assertions),
            }
        }

        struct WasmLogger;

        impl Log for WasmLogger {
            fn enabled(&self, metadata: &Metadata) -> bool {
                if !log_flag_enabled() {
                    return false;
                }
                metadata.level() <= Level::Info
            }

            fn log(&self, record: &Record) {
                if self.enabled(record.metadata()) {
                    let now = js_sys::Date::new_0().to_iso_string();
                    web_sys::console::log_1(&format!("[{}] {} - {}", now, record.level(), record.args()).into());
                }
            }

            fn flush(&self) {}
        }

        static LOGGER: WasmLogger = WasmLogger;
        if log::set_logger(&LOGGER).is_ok() {
            log::set_max_level(LevelFilter::Info);
            console_error_panic_hook::set_once();
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use pretty_env_logger::formatted_timed_builder;
        let mut log_builder = formatted_timed_builder();

        let file_log = std::env::var("FILE_LOG").ok().filter(|v| !v.is_empty());
        if let Ok(filter_env) = std::env::var("RUST_LOG") {
            log_builder.parse_filters(&filter_env);
        } else if let Some(level) = file_log.as_deref() {
            log_builder.parse_filters(level);
        } else {
            log_builder.filter(None, log::LevelFilter::Info);
        }
        // Suppress noisy third-party log targets
        log_builder.filter_module("rig::completions", log::LevelFilter::Off);
        log_builder.filter_module("rig::agent_chat", log::LevelFilter::Off);
        log_builder.filter_module("sqlx::query", log::LevelFilter::Off);
        // Keep exact provider payload tracing available without a separate feature flag.
        log_builder.filter_module("llm::request", log::LevelFilter::Debug);
        log_builder.filter_module("llm::raw_request", log::LevelFilter::Debug);
        log_builder.filter_module("llm::raw_response", log::LevelFilter::Debug);

        // FILE_LOG=<level> tees the (uncolored) stream to a file at FILE_LOG_PATH
        // (default: <binary>.log in the cwd) so live runs have a grep-able log.
        if file_log.is_some() {
            if let Some(target) = file_log_target() {
                log_builder.target(target);
                log_builder.write_style(env_logger::WriteStyle::Never);
            }
        }

        let _ = log_builder.try_init();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn file_log_target() -> Option<env_logger::Target> {
    let path = std::env::var("FILE_LOG_PATH").ok().filter(|p| !p.is_empty()).unwrap_or_else(default_file_log_path);
    if let Some(parent) = std::path::Path::new(&path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        Ok(file) => Some(env_logger::Target::Pipe(Box::new(FileTee { file }))),
        Err(e) => {
            eprintln!("FILE_LOG: cannot open log file '{path}': {e}; file logging disabled");
            None
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn default_file_log_path() -> String {
    // Mirror the launcher's builder layout (logs/sandbox-<id>.log): keyed by SANDBOX_ID
    // when present, else the binary name.
    let stem = std::env::var("SANDBOX_ID")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|id| format!("sandbox-{id}"))
        .unwrap_or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                .unwrap_or_else(|| "app".to_string())
        });
    format!("logs/{stem}.log")
}

#[cfg(not(target_arch = "wasm32"))]
struct FileTee {
    file: std::fs::File,
}

#[cfg(not(target_arch = "wasm32"))]
impl std::io::Write for FileTee {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = std::io::stderr().write_all(buf);
        self.file.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let _ = std::io::stderr().flush();
        self.file.flush()
    }
}

/// Aggregates repeated messages and periodically emits occurrence counts.
pub struct ThrottleLogger {
    messages: Arc<Mutex<HashMap<String, usize>>>,
    join_handle: JoinHandle<()>,
}

impl ThrottleLogger {
    /// Starts an aggregation interval under the supplied log target.
    pub fn new(namespace: String, delay: Duration) -> Self {
        let messages = Arc::new(Mutex::new(HashMap::new()));
        let messages_clone = messages.clone();
        let ns = namespace.clone();

        let join_handle = spawn(async move {
            let mut interval = time::interval(delay);
            interval.tick().await;

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

    /// Increments the count for an exact message string.
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
