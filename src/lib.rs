#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]
//! Opt-in runtime and infrastructure building blocks used by Midwess systems.
//!
//! Hero keeps architectural boundary code consistent without imposing an
//! application framework. Its modules cover cancellation, resource pooling,
//! service discovery and transport, object storage, email, tokens, logging,
//! identifiers, and browser integration.
//!
//! # Feature selection
//!
//! The default feature set is empty. Enable only the capabilities a binary or
//! library imports:
//!
//! ```toml
//! [dependencies]
//! hero = {
//!     package = "midwess-hero",
//!     version = "1",
//!     features = ["cancellation", "task"]
//! }
//! ```
//!
//! Native-only modules are omitted from `wasm32` builds. The `wasm` module is
//! available only on WebAssembly targets. Cross-platform modules expose the
//! same public path on native and WebAssembly, while relaxing thread-safety
//! bounds where a browser runtime cannot move work between threads.
//!
//! # Where to start
//!
//! - Use `cancellation` for structured cancellation trees and scoped work.
//! - Use `pool` for bounded, automatically returned infrastructure resources.
//! - Use `distributed-id` for process-local Snowflake-style identifiers, and
//!   add `distributed-id-etcd` when worker IDs must be coordinated across a
//!   fleet.
//! - Use `api-gateway`, `grpc-gateway`, `grpc-server`, and `tcp` at service
//!   network boundaries.
//! - Use `s3`, `smtp`, and `token` for concrete infrastructure integrations.
//! - Use `wasm`, `stream`, and `time` in browser-facing clients.
//!
//! Each module documents its runtime behavior, failure modes, and intended
//! composition with related features.

#[cfg(all(feature = "api-gateway", not(target_family = "wasm")))]
#[cfg_attr(docsrs, doc(cfg(feature = "api-gateway")))]
pub mod api_gateway;
#[cfg(feature = "cancellation")]
#[cfg_attr(docsrs, doc(cfg(feature = "cancellation")))]
pub mod cancellation;
#[cfg(feature = "config")]
#[cfg_attr(docsrs, doc(cfg(feature = "config")))]
pub mod config;
#[cfg(feature = "distributed-id")]
#[cfg_attr(docsrs, doc(cfg(feature = "distributed-id")))]
pub mod distributed_id;
#[cfg(feature = "env")]
#[cfg_attr(docsrs, doc(cfg(feature = "env")))]
pub mod env;
#[cfg(feature = "finally")]
#[cfg_attr(docsrs, doc(cfg(feature = "finally")))]
pub mod finally;
#[cfg(all(feature = "grpc-gateway", not(target_family = "wasm")))]
#[cfg_attr(docsrs, doc(cfg(feature = "grpc-gateway")))]
pub mod grpc_gateway;
#[cfg(feature = "logger")]
#[cfg_attr(docsrs, doc(cfg(feature = "logger")))]
pub mod logger;
#[cfg(feature = "maybe")]
#[cfg_attr(docsrs, doc(cfg(feature = "maybe")))]
pub mod maybe;
#[cfg(feature = "never-send")]
#[cfg_attr(docsrs, doc(cfg(feature = "never-send")))]
pub mod never_send;
#[cfg(feature = "number")]
#[cfg_attr(docsrs, doc(cfg(feature = "number")))]
pub mod number;
#[cfg(feature = "pool")]
#[cfg_attr(docsrs, doc(cfg(feature = "pool")))]
pub mod pool;
#[cfg(feature = "retry")]
#[cfg_attr(docsrs, doc(cfg(feature = "retry")))]
pub mod retry;
#[cfg(all(feature = "s3", not(target_family = "wasm")))]
#[cfg_attr(docsrs, doc(cfg(feature = "s3")))]
pub mod s3;
#[cfg(all(feature = "shell", not(target_family = "wasm")))]
#[cfg_attr(docsrs, doc(cfg(feature = "shell")))]
pub mod shell;
#[cfg(all(feature = "smtp", not(target_family = "wasm")))]
#[cfg_attr(docsrs, doc(cfg(feature = "smtp")))]
pub mod smtp;
#[cfg(feature = "stream")]
#[cfg_attr(docsrs, doc(cfg(feature = "stream")))]
pub mod stream;
#[cfg(feature = "string")]
#[cfg_attr(docsrs, doc(cfg(feature = "string")))]
pub mod string;
#[cfg(all(feature = "task", not(target_family = "wasm")))]
#[cfg_attr(docsrs, doc(cfg(feature = "task")))]
pub mod task;
#[cfg(all(feature = "tcp", not(target_family = "wasm")))]
#[cfg_attr(docsrs, doc(cfg(feature = "tcp")))]
pub mod tcp;
#[cfg(feature = "throttle-future")]
#[cfg_attr(docsrs, doc(cfg(feature = "throttle-future")))]
pub mod throttle_future;
#[cfg(feature = "time")]
#[cfg_attr(docsrs, doc(cfg(feature = "time")))]
pub mod time;
#[cfg(all(feature = "token", not(target_family = "wasm")))]
#[cfg_attr(docsrs, doc(cfg(feature = "token")))]
pub mod token;
#[cfg(all(feature = "wasm", target_family = "wasm"))]
#[cfg_attr(docsrs, doc(cfg(feature = "wasm")))]
pub mod wasm;
#[cfg(feature = "yield-container")]
#[cfg_attr(docsrs, doc(cfg(feature = "yield-container")))]
pub mod yield_container;
