use core_services::db::SurrealDbConnectionInfo;
use lazy_static::lazy_static;
use std::env;
use std::time::Duration;

#[derive(Debug)]
pub struct KongGatewayConfig {
    pub admin_url: String,
    pub url: String
}

impl Default for KongGatewayConfig {
    fn default() -> Self {
        Self {
            admin_url: env::var("DEVLOG_KONG_GATEWAY_ADMIN_URL").unwrap_or("http://localhost:8001".to_owned()),
            url: env::var("DEVLOG_KONG_GATEWAY_URL").unwrap_or("http://localhost:8000".to_owned())
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

#[derive(Debug)]
pub struct Config {
    pub token: TokenConfig,
    pub surrealdb: SurrealDbConnectionInfo,
    pub kong: KongGatewayConfig
}

impl Default for Config {
    fn default() -> Self {
        Self {
            token: Default::default(),
            kong: Default::default(),
            surrealdb: SurrealDbConnectionInfo {
                socket_address: env::var("DEVLOG_SURREAL_DB_SOCKET_ADDRESS").unwrap_or("127.0.0.1:8000".to_owned()),
                db_name: env::var("DEVLOG_SURREAL_DB_NAME").unwrap_or("devlog".to_owned()),
                namespace: env::var("DEVLOG_SURREAL_DB_NAMESPACE").unwrap_or("dev".to_owned()),
                db_username: env::var("DEVLOG_SURREAL_DB_USERNAME").unwrap_or("root".to_owned()),
                db_password: env::var("DEVLOG_SURREAL_DB_PASSWORD").unwrap_or("root".to_owned())
            }
        }
    }
}

lazy_static! {
    pub static ref CONFIGS: Config = Default::default();
}
