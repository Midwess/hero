#[cfg(feature = "db-remote")]
use surrealdb::engine::remote::ws::Client;

#[cfg(feature = "s3")]
pub type S3Connection = aws_sdk_s3::client::Client;

#[cfg(feature = "smtp")]
pub type SmtpTransport = lettre::SmtpTransport;

#[cfg(feature = "grpc-server")]
pub mod grpc;

#[cfg(feature = "host_machine")]
pub mod host_machine;

#[cfg(feature = "retry")]
pub mod retry;

#[cfg(feature = "s3")]
pub mod s3;

#[cfg(feature = "smtp")]
pub mod smtp;

#[cfg(feature = "db-remote")]
pub type Db = surrealdb::Surreal<Client>;

#[cfg(feature = "local-storage")]
pub mod local_storage;

pub mod config;
#[cfg(any(feature = "db-local", feature = "db-remote"))]
pub mod db;
pub mod logger;
pub mod services;
pub mod utils;
