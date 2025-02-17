use surrealdb::engine::remote::ws::Client;
use surrealdb::Surreal;

#[cfg(feature = "s3")]
pub type S3Connection = aws_sdk_s3::client::Client;

#[cfg(feature = "smtp")]
pub type SmtpTransport = lettre::SmtpTransport;

#[cfg(feature = "config")]
pub mod config;
#[cfg(feature = "db")]
pub mod db;
#[cfg(feature = "grpc-server")]
pub mod grpc;
#[cfg(feature = "host_machine")]
pub mod host_machine;
pub mod logger;
#[cfg(feature = "retry")]
pub mod retry;
#[cfg(feature = "s3")]
pub mod s3;
#[cfg(feature = "services")]
pub mod services;
#[cfg(feature = "smtp")]
pub mod smtp;
pub mod utils;

#[cfg(feature = "db")]
pub type Db = Surreal<Client>;
