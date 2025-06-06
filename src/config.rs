use lazy_static::lazy_static;
use std::env;
use std::time::Duration;

#[derive(Debug)]
pub struct TokenConfig {
    pub jwt_refresh_token_private_key: String,
    pub jwt_access_token_private_key: String,
    pub access_token_duration: Duration,
    pub refresh_token_duration: Duration
}

#[derive(Debug, Clone)]
pub struct Crafter {
    pub url: String
}

impl Default for Crafter {
    fn default() -> Self {
        Self {
            url: env::var("DEVLOG_CRAFTER_URL").unwrap_or("http://localhost:8025".to_owned())
        }
    }
}

#[derive(Debug, Clone)]
pub struct Smtp {
    pub host: String,
    pub username: String,
    pub password: String,
    pub port: i32
}

impl Default for Smtp {
    fn default() -> Self {
        Self {
            host: env::var("DEVLOG_SMTP_HOST").unwrap_or("127.0.0.1".to_owned()),
            port: env::var("DEVLOG_SMTP_PORT")
                .map(|it| it.parse().expect("DEVLOG_SMTP_PORT must be a number"))
                .unwrap_or(8025),
            username: env::var("DEVLOG_SMTP_USERNAME").unwrap_or("root".to_owned()),
            password: env::var("DEVLOG_SMTP_PASSWORD").unwrap_or("root".to_owned())
        }
    }
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
    pub smtp: Smtp,
    pub crafter: Crafter
}

lazy_static! {
    pub static ref CONFIGS: Config = Default::default();
}
