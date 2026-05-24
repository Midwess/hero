use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Local,
    Dev,
    Staging,
    Production
}

static CURRENT: OnceLock<Environment> = OnceLock::new();

impl Environment {
    pub fn current() -> Environment {
        *CURRENT.get_or_init(Self::from_env_var)
    }

    fn from_env_var() -> Environment {
        let raw = std::env::var("ENVIRONMENT").or_else(|_| std::env::var("DEPLOYMENT_ENV")).unwrap_or_default();
        match raw.to_ascii_lowercase().as_str() {
            "" | "local" => Environment::Local,
            "dev" | "development" => Environment::Dev,
            "staging" | "stage" => Environment::Staging,
            "prod" | "production" => Environment::Production,
            _ => Environment::Local
        }
    }

    pub fn is_local(self) -> bool {
        matches!(self, Environment::Local)
    }

    pub fn is_dev_or_local(self) -> bool {
        matches!(self, Environment::Local | Environment::Dev)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Dev => "dev",
            Environment::Staging => "staging",
            Environment::Production => "production"
        }
    }
}
