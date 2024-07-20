use surrealdb::{engine::remote::ws::Client, Surreal};

pub mod services;
pub mod s3;
pub mod config;
pub mod grpc;
pub mod logger;

use once_cell::sync::Lazy;

pub type Db = Surreal<Client>;
pub static DB: Lazy<Db> = Lazy::new(Surreal::init);
