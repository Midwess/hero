//! Browser integrations for HTTP, byte conversion, and OPFS access handles.
//!
//! This module exists only on WebAssembly targets. [`HttpClient`] can create
//! either Fetch or XMLHttpRequest transports; choose XHR when upload progress is
//! required and Fetch when streaming bodies or direct `Response` access matter.
//!
//! # Example
//!
//! ```no_run
//! # async fn example() -> Result<(), wasm_bindgen::JsValue> {
//! let (status, headers, body) = hero::wasm::HttpClient::new()
//!     .url("/api/profile")
//!     .fetch()?
//!     .bytes()
//!     .await?;
//! assert_eq!(status, 200);
//! # let _ = (headers, body);
//! # Ok(())
//! # }
//! ```

/// Conversion helpers for JavaScript byte and file types.
pub mod extensions;
/// Fetch and XMLHttpRequest builders.
pub mod http;
/// Async writer adapter for synchronous OPFS access handles.
pub mod opfs;

pub use http::*;
