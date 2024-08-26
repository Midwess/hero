use std::ops::Deref;
use async_trait::async_trait;

use crate::{s3::S3Client, services::errors::Errors, smtp::client::SmtpClient};

pub type NoParam = ();

pub type VoidResponse = ();

pub type Resolve<T> = Result<T, Errors>;

pub fn error_message<T>(resolve: Option<Resolve<T>>) -> Option<String> {
    if resolve.is_none() {
        return None;
    }

    resolve.unwrap().err().map(|it| it.to_string())
}

#[derive(Clone, Debug)]
pub struct ClonedResult<T, E> where T: Clone, E: Clone {
    result: Result<T, E>
}

impl<T, E> From<Result<T, E>> for ClonedResult<T, E> where T: Clone, E: Clone {
    fn from(value: Result<T, E>) -> Self {
        Self {
            result: value
        }
    }
}

impl<T, E> Deref for ClonedResult<T, E> where T: Clone, E: Clone {
    type Target = Result<T, E>;

    fn deref(&self) -> &Self::Target {
        &self.result
    }
}

pub trait Response {}

#[async_trait]
pub trait Service<P, T>
    where
        P: Clone,
{
    async fn execute(&self, params: P) -> Resolve<T>;
}

pub trait MyTrait: Send + Sync {}

pub struct UserService<'a> {
    pub user_repository: &'a Box<dyn MyTrait>,
    pub s3: &'a S3Client,
    pub smtp_client: &'a SmtpClient,
}

pub struct XXService<'a> {
    repo: &'a Box<dyn MyTrait + Send + Sync>,
    smtp: &'a SmtpClient,
    x: &'a str
}

#[async_trait]
impl<'a> Service<(), ()> for UserService<'a> {
   async fn execute(&self, _param: ()) -> Resolve<()> {
       Ok(())
   } 
}

