use surrealdb::{engine::remote::ws::Client, Surreal};

pub mod services;
pub mod config;
pub mod grpc;

use once_cell::sync::Lazy;

pub type Db = Surreal<Client>;
pub static DB: Lazy<Db> = Lazy::new(Surreal::init);
