//! Retry helpers for explicitly classified transient failures.
//!
//! [`crate::retry!`] performs a fixed number of attempts with a fixed delay and
//! works on native and WASM targets. Native code can use `with_capped_backoff` for
//! indefinite exponential backoff with jitter.

#[cfg(not(target_family = "wasm"))]
/// Capped exponential backoff for native Tokio runtimes.
pub mod backoff;
/// Definition of the [`retry!`](crate::retry!) macro.
pub mod macros;

#[cfg(not(target_family = "wasm"))]
pub use backoff::with_capped_backoff;
