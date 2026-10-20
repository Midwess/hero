//! Bounded asynchronous resource pools with automatic return-on-drop.
//!
//! A [`allocator::PoolBuilder`] eagerly creates the configured minimum number
//! of resources. [`request::PoolRequest`] either checks one out immediately or
//! waits for another borrower to drop its [`response::PoolResponse`]. Resources
//! above the minimum are removed after their idle timeout.
//!
//! The response guard dereferences to the resource and returns it asynchronously
//! when dropped. Keep a runtime alive long enough for that return task to run.
//!
//! # Example
//!
//! ```no_run
//! # futures::executor::block_on(async {
//! use hero::pool::allocator::{PoolBuilder, PoolResourceProvider};
//! use hero::pool::request::PoolRequestBuilder;
//!
//! struct Strings;
//!
//! #[async_trait::async_trait]
//! impl PoolResourceProvider<String> for Strings {
//!     async fn create(&self) -> String {
//!         String::new()
//!     }
//! }
//!
//! let pool = PoolBuilder::new(Box::new(Strings))
//!     .min_pool_size(1)
//!     .max_pool_size(4)
//!     .build()
//!     .await
//!     .unwrap();
//! let request = PoolRequestBuilder::new().pool(pool).build().unwrap();
//! let mut value = request.retrieve().await.unwrap();
//! value.push_str("reused");
//! # });
//! ```

/// Pool construction, resource providers, and low-level allocation.
pub mod allocator;
/// Reusable checkout requests with optional wait timeouts.
pub mod request;
/// Return-on-drop checkout guards.
pub mod response;
