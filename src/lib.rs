extern crate core;

#[cfg(any(feature = "db-local", feature = "db-remote"))]
use surrealdb::engine::any::Any;

#[cfg(feature = "s3")]
pub type S3Connection = aws_sdk_s3::client::Client;

#[cfg(feature = "smtp")]
pub type SmtpTransport = lettre::SmtpTransport;

#[cfg(feature = "host_machine")]
pub mod host_machine;

#[cfg(feature = "retry")]
pub mod retry;

#[cfg(feature = "s3")]
pub mod s3;

#[cfg(feature = "smtp")]
pub mod smtp;

#[cfg(any(feature = "db-local", feature = "db-remote"))]
pub type Db = surrealdb::Surreal<Any>;

pub mod local_storage;

pub mod config;
pub mod db;
pub mod logger;
pub mod services;
pub mod utils;