use lazy_static::lazy_static;
use std::{env, time::Duration};

#[derive(Debug)]
pub struct TokenConfig {
    pub jwt_refresh_token_private_key: String,
    pub jwt_access_token_private_key: String,
    pub access_token_duration: Duration,
    pub refresh_token_duration: Duration
}

impl Default for TokenConfig {
    fn default() -> Self {
        let refresh_token_duration = Duration::from_millis(env::var("DEVLOGS_REFRESH_TOKEN_LIFETIME_IN_MS")
            .map(|env_var| env_var.parse().expect("The DEVLOGS_REFRESH_TOKEN_LIFETIME_IN_MS must be number"))
            .unwrap_or(86400000)); // 1 day
        Self {
            jwt_refresh_token_private_key: env::var("DEVLOGS_REFRESH_TOKEN_PRIVATE_KEY")
                .unwrap_or("this_is_unsafe_keythis_is_unsafe_keythis_is_unsafe_key".to_owned()),
            jwt_access_token_private_key: env::var("DEVLOGS_ACCESS_TOKEN_PRIVATE_KEY")
                .unwrap_or("this_is_unsafe_keythis_is_unsafe_keythis_is_unsafe_key".to_owned()),
            access_token_duration: Duration::from_millis(env::var("DEVLOGS_ACCESS_TOKEN_LIFETIME_IN_MS")
                .map(|env_var| env_var.parse().expect("The DEVLOGS_ACCESS_TOKEN_LIFETIME_IN_MS must be number"))
                .unwrap_or(900000)), // 15 minutes
            refresh_token_duration
        }
    }
}

#[derive(Debug, Default)]
pub struct Config {
    pub token: TokenConfig,
}

lazy_static! {
    pub static ref CONFIGS: Config = Default::default();
}
