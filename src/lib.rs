#![feature(duration_constructors)]

use s3::S3Client;
use surrealdb::{engine::remote::ws::Client, Surreal};

pub mod services;
pub mod s3;
pub mod config;
pub mod grpc;
pub mod logger;
pub mod utils;
pub mod smtp;

use once_cell::sync::{Lazy};
use tokio::sync::OnceCell;

pub type Db = Surreal<Client>;
pub static DB: Lazy<Db> = Lazy::new(Surreal::init);
pub static S3_CLIENT: OnceCell<S3Client> = OnceCell::const_new();
