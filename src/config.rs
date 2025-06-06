use lazy_static::lazy_static;
use std::env;

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

pub struct Config {
    pub kong: KongGatewayConfig,
    pub surrealdb_ws_address: String,
    pub surrealdb_password: String,
    pub environment: String
}

impl Default for Config {
    fn default() -> Self {
        Self {
            kong: KongGatewayConfig::default(),
            surrealdb_ws_address: env::var("DEVLOG_SURREALDB_WS_ADDRESS").unwrap_or("ws://localhost:8500".to_owned()),
            surrealdb_password: env::var("DEVLOG_SURREALDB_PASSWORD").unwrap_or("ssh".to_owned()),
            environment: env::var("DEVLOG_ENVIRONMENT").unwrap_or("development".to_owned())
        }
    }
}
lazy_static! {
    pub static ref CONFIGS: Config = Default::default();
}
