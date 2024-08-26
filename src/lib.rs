#![feature(duration_constructors)]
#![feature(type_alias_impl_trait)]

use surrealdb::{engine::remote::ws::Client, Surreal};

pub mod services;
pub mod s3;
pub mod config;
pub mod grpc;
pub mod logger;
pub mod utils;
pub mod smtp;
pub mod db;

pub type Db = Surreal<Client>;

