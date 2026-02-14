use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Default)]
pub enum DeploymentRegion {
    #[default]
    AU,
    US,
    EU,
    AP
}

impl DeploymentRegion {
    pub fn from_string(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "AP" | "SIN" => Self::AP,
            "US" => Self::US,
            "EU" => Self::EU,
            _ => Self::AU
        }
    }
}
