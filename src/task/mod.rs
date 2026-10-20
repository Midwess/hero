//! Ownership guards for spawned Tokio tasks.
//!
//! # Example
//!
//! ```no_run
//! use hero::task::AbortOnDrop;
//!
//! # async fn example() {
//! let handle = tokio::spawn(async {
//!     run_background_work().await;
//! });
//! let _guard = AbortOnDrop::new(handle);
//! # }
//! # async fn run_background_work() {}
//! ```

/// Abort-on-drop task guard.
pub mod abort_on_drop;

pub use abort_on_drop::AbortOnDrop;
