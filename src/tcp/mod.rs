//! Dual-stack TCP, UDP, and optional tonic listener binding.
//!
//! # Example
//!
//! ```no_run
//! # async fn example() -> hero::tcp::listener::Result<()> {
//! let connection = hero::tcp::listener::find_tcp_listener(Some(3000)).await?;
//! println!("{}:{}", connection.public_host, connection.port);
//! # Ok(())
//! # }
//! ```

/// Listener discovery and connection metadata.
pub mod listener;
