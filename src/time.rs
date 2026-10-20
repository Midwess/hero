//! Cross-platform Unix epoch time with microsecond units.
//!
//! Native builds use [`std::time::SystemTime`]. WASM builds combine the wall
//! clock with `Performance.now()` when available, falling back to `Date.now()`.

use std::sync::atomic::AtomicU64;
use std::sync::Once;

#[allow(dead_code)]
static INIT: Once = Once::new();
#[allow(dead_code)]
static EPOCH_OFFSET_MICRO: AtomicU64 = AtomicU64::new(0);
#[allow(dead_code)]
static PERF_START_MICRO: AtomicU64 = AtomicU64::new(0);

#[cfg(target_arch = "wasm32")]
fn init_epoch() {
    use std::sync::atomic::Ordering;
    INIT.call_once(|| {
        let js_epoch_ms = js_sys::Date::now();
        let Some(performance) = web_sys::window().and_then(|window| window.performance()) else {
            EPOCH_OFFSET_MICRO.store((js_epoch_ms * 1000.0) as u64, Ordering::SeqCst);
            PERF_START_MICRO.store(0, Ordering::SeqCst);
            return;
        };
        let perf_ms = performance.now();

        // Convert both to microseconds
        let js_epoch_micro = (js_epoch_ms * 1000.0) as u64;
        let perf_micro = (perf_ms * 1000.0) as u64;

        // epoch = epoch_base + (current_perf - perf_base)
        EPOCH_OFFSET_MICRO.store(js_epoch_micro - perf_micro, Ordering::SeqCst);
        PERF_START_MICRO.store(perf_micro, Ordering::SeqCst);
    });
}

#[cfg(target_arch = "wasm32")]
fn epoch_from_js_fallback() -> u64 {
    (js_sys::Date::now() * 1000.0) as u64
}

#[cfg(target_arch = "wasm32")]
/// Returns microseconds elapsed since the Unix epoch.
///
/// Browser precision and privacy settings may reduce the effective resolution.
pub fn epoch_micro() -> u64 {
    use std::sync::atomic::Ordering;
    init_epoch();

    // Get window
    let window = match web_sys::window() {
        Some(w) => w,
        None => return epoch_from_js_fallback(),
    };

    // Get performance
    let perf = match window.performance() {
        Some(p) => p,
        None => return epoch_from_js_fallback(),
    };

    // High-resolution timer
    let perf_ms = perf.now();
    let perf_now_micro = (perf_ms * 1000.0) as u64;

    // Offset established at init
    let offset = EPOCH_OFFSET_MICRO.load(Ordering::SeqCst);

    (offset + perf_now_micro) as u64
}

#[cfg(not(target_arch = "wasm32"))]
/// Returns microseconds elapsed since the Unix epoch.
///
/// Times before the epoch are represented as zero rather than panicking.
pub fn epoch_micro() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();

    (dur.as_secs()) * 1_000_000 + (dur.subsec_nanos() as u64) / 1_000
}
