use lazy_static::lazy_static;
use std::env;

#[derive(Debug, Clone)]
pub struct Crafter {
    pub url: String
}

impl Default for Crafter {
    fn default() -> Self {
        Self {
            url: env::var("DEVLOG_CRAFTER_URL").unwrap_or("http://localhost:8000/crafter".to_owned())
        }
    }
}

#[derive(Debug, Default)]
pub struct Config {
    pub crafter: Crafter
}

lazy_static! {
    pub static ref CONFIGS: Config = Default::default();
}
