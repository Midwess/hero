#![feature(duration_constructors)]
#![feature(type_alias_impl_trait)]

use s3::S3Client;
use surrealdb::{engine::remote::ws::Client, Surreal};

pub type S3Connection = aws_sdk_s3::client::Client;
pub type SmtpTransport = lettre::SmtpTransport;

pub mod services;
pub mod repository;
pub mod s3;
pub mod config;
pub mod grpc;
pub mod logger;
pub mod utils;
pub mod smtp;
pub mod db;

pub type Db = Surreal<Client>;

