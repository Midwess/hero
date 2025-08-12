use lazy_static::lazy_static;
use std::env;

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

#[derive(Debug, Default)]
pub struct Config {
    pub smtp: Smtp,
}

lazy_static! {
    pub static ref CONFIGS: Config = Default::default();
}
