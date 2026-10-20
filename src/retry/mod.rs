#[cfg(not(target_family = "wasm"))]
pub mod backoff;
pub mod macros;

#[cfg(not(target_family = "wasm"))]
pub use backoff::with_capped_backoff;
