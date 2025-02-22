#[cfg(feature = "db-remote")]
use core_services::db::remote_surrealdb::SurrealDbConnectionInfo;
use lazy_static::lazy_static;
use std::env;
use std::time::Duration;

#[derive(Debug)]
pub struct KongGatewayConfig {
    pub admin_url: String,
    pub url: String,
    pub host: String
}

impl Default for KongGatewayConfig {
    fn default() -> Self {
        Self {
            admin_url: env::var("DEVLOG_KONG_GATEWAY_ADMIN_URL").unwrap_or("http://localhost:8001".to_owned()),
            url: env::var("DEVLOG_KONG_GATEWAY_URL").unwrap_or("http://localhost:8000".to_owned()),
            host: env::var("DEVLOG_KONG_GATEWAY_HOST").unwrap_or("localhost".to_owned())
        }
    }
}

#[derive(Debug)]
pub struct TokenConfig {
    pub jwt_refresh_token_private_key: String,
    pub jwt_access_token_private_key: String,
    pub access_token_duration: Duration,
    pub refresh_token_duration: Duration
}

impl Default for TokenConfig {
    fn default() -> Self {
        let refresh_token_duration = Duration::from_millis(
            env::var("DEVLOG_REFRESH_TOKEN_LIFETIME_IN_MS")
                .map(|env_var| env_var.parse().expect("The DEVLOG_REFRESH_TOKEN_LIFETIME_IN_MS must be number"))
                .unwrap_or(86400000)
        ); // 1 day
        Self {
            jwt_refresh_token_private_key: env::var("DEVLOG_REFRESH_TOKEN_PRIVATE_KEY")
                .unwrap_or("this_is_unsafe_keythis_is_unsafe_keythis_is_unsafe_key".to_owned()),
            jwt_access_token_private_key: env::var("DEVLOG_ACCESS_TOKEN_PRIVATE_KEY")
                .unwrap_or("this_is_unsafe_keythis_is_unsafe_keythis_is_unsafe_key".to_owned()),
            access_token_duration: Duration::from_millis(
                env::var("DEVLOG_ACCESS_TOKEN_LIFETIME_IN_MS")
                    .map(|env_var| env_var.parse().expect("The DEVLOG_ACCESS_TOKEN_LIFETIME_IN_MS must be number"))
                    .unwrap_or(1000 * 60 * 60 * 24 * 256)
            ),
            refresh_token_duration
        }
    }
}

#[derive(Debug, Default)]
pub struct Config {
    pub token: TokenConfig,
    #[cfg(feature = "db-remote")]
    pub surrealdb: SurrealDbConnectionInfo,
    pub kong: KongGatewayConfig
}

lazy_static! {
    pub static ref CONFIGS: Config = Default::default();
}
