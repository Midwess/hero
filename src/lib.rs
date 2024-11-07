use surrealdb::engine::remote::ws::Client;
use surrealdb::Surreal;

pub type S3Connection = aws_sdk_s3::client::Client;
pub type SmtpTransport = lettre::SmtpTransport;

pub mod config;
pub mod db;
pub mod grpc;
pub mod host_machine;
pub mod logger;
pub mod s3;
pub mod services;
pub mod smtp;
pub mod utils;

pub type Db = Surreal<Client>;
