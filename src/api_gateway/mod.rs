//! Kong-oriented service and route registration.
//!
//! Build one [`service::GatewayService`] from one or more
//! [`service::GatewayRoute`] values, then apply it through an
//! [`client::ApiGatewayClient`]. The concrete [`kong::client::KongGatewayAdminClient`]
//! reconciles services, routes, CORS, gRPC-Web, and request headers through the
//! Kong Admin API.
//!
//! # Example
//!
//! ```
//! use hero::api_gateway::service::{
//!     GatewayRouteBuilder, GatewayRouteExpression, GatewayServiceBuilder,
//! };
//!
//! let route = GatewayRouteBuilder::new()
//!     .name("users")
//!     .path(GatewayRouteExpression::exact_or_subpath("/users"))
//!     .http()
//!     .build()?;
//! let service = GatewayServiceBuilder::new()
//!     .name("user service")
//!     .http("users".into(), 3000)
//!     .routes(vec![route])
//!     .build()?;
//!
//! assert_eq!(service.name, "user-service");
//! # Ok::<(), hero::api_gateway::service::BuildError>(())
//! ```

/// Backend-neutral gateway operations.
pub mod client;
/// Errors shared by gateway clients.
pub mod error;
/// Kong Admin API implementation.
pub mod kong;
/// Service and route definitions with validating builders.
pub mod service;

pub use error::{Error, Result};
