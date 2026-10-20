//! Normalized deployment-environment detection.
//!
//! The first call to [`Environment::current`] reads `ENVIRONMENT`, then
//! `DEPLOYMENT_ENV`, and caches the result for the process lifetime.

use std::sync::OnceLock;

/// Deployment stages understood by shared Midwess code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    /// Local development, including an unset or unrecognized environment.
    Local,
    /// A shared development deployment.
    Dev,
    /// A pre-production staging deployment.
    Staging,
    /// A production deployment.
    Production,
}

static CURRENT: OnceLock<Environment> = OnceLock::new();

impl Environment {
    /// Returns the cached process environment.
    ///
    /// Accepted aliases are `local`, `dev`, `development`, `staging`, `stage`,
    /// `prod`, and `production`; matching is ASCII case-insensitive.
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
            _ => Environment::Local,
        }
    }

    /// Reports whether this is [`Environment::Local`].
    pub fn is_local(self) -> bool {
        matches!(self, Environment::Local)
    }

    /// Reports whether development-only behavior is appropriate.
    pub fn is_dev_or_local(self) -> bool {
        matches!(self, Environment::Local | Environment::Dev)
    }

    /// Returns the canonical lowercase name used in logs and metadata.
    pub fn as_str(self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Dev => "dev",
            Environment::Staging => "staging",
            Environment::Production => "production",
        }
    }
}
