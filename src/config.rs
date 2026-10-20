//! Conventional process configuration for Midwess gateway-aware services.
//!
//! [`CONFIGS`] reads the environment once, on first access. Construct
//! [`Config::default`] directly when tests or short-lived tools need a fresh
//! snapshot after changing environment variables.

use std::env;
use std::sync::LazyLock;

/// Connection and public-address settings for the shared Kong gateway.
///
/// Missing or malformed values fall back to local-development defaults. This
/// makes the configuration suitable for development processes; production
/// deployments should set the variables explicitly.
#[derive(Debug, Clone)]
pub struct KongGatewayConfig {
    /// Kong Admin API base URL, from `KONG_GATEWAY_ADMIN_URL`.
    pub admin_url: String,
    /// Gateway base URL used by service clients, from `KONG_GATEWAY_URL`.
    pub url: String,
    /// Internal gateway host, from `KONG_GATEWAY_HOST`.
    pub host: String,
    /// Internal gateway port, from `KONG_GATEWAY_PORT`.
    pub port: u16,
    /// Externally reachable gateway host, from `PUBLIC_GATEWAY_HOST`.
    pub public_host: String,
    /// Externally reachable gateway port, from `PUBLIC_GATEWAY_PORT`.
    pub public_port: u16,
    /// Whether [`KongGatewayConfig::public_url`] uses HTTPS.
    pub public_with_ssl: bool,
}

impl Default for KongGatewayConfig {
    fn default() -> Self {
        Self {
            admin_url: env::var("KONG_GATEWAY_ADMIN_URL").unwrap_or("http://localhost:8001".to_owned()),
            url: env::var("KONG_GATEWAY_URL").unwrap_or("http://localhost:8000".to_owned()),
            host: env::var("KONG_GATEWAY_HOST").unwrap_or("localhost".to_owned()),
            port: env::var("KONG_GATEWAY_PORT").unwrap_or("8000".to_owned()).parse().unwrap_or(8000),
            public_host: env::var("PUBLIC_GATEWAY_HOST").unwrap_or("localhost".to_owned()),
            public_port: env::var("PUBLIC_GATEWAY_PORT").ok().and_then(|it| it.parse().ok()).unwrap_or(8000),
            public_with_ssl: env::var("PUBLIC_GATEWAY_WITH_SSL").ok().and_then(|it| it.parse().ok()).unwrap_or(false),
        }
    }
}

impl KongGatewayConfig {
    /// Formats the externally reachable gateway origin.
    ///
    /// The configured port is always included, even when it is the scheme's
    /// conventional port.
    pub fn public_url(&self) -> String {
        if self.public_with_ssl {
            format!("https://{}:{}", self.public_host, self.public_port)
        } else {
            format!("http://{}:{}", self.public_host, self.public_port)
        }
    }
}

/// Top-level configuration shared by Hero integrations.
pub struct Config {
    /// Kong gateway settings.
    pub kong: KongGatewayConfig,
    /// Deployment name from `ENVIRONMENT`, defaulting to `development`.
    pub environment: String,
}

impl Default for Config {
    fn default() -> Self {
        let environment = env::var("ENVIRONMENT").unwrap_or("development".to_owned());
        Self {
            kong: KongGatewayConfig::default(),
            environment,
        }
    }
}

/// Lazily initialized process-wide configuration snapshot.
///
/// The first access fixes the values for the lifetime of the process.
pub static CONFIGS: LazyLock<Config> = LazyLock::new(Config::default);
