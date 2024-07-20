use schema::devlog::entities::User;
use log::*;
use crate::services::{base::Service, errors::Errors, token::{ReadFrom, TokenService}};

use tonic::{async_trait, Status};
use tonic::codegen::http::Request;
use tonic::transport::Body;
use tonic_middleware::RequestInterceptor;
#[derive(Clone)]
pub struct AuthInterceptor {}

impl AuthInterceptor {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl RequestInterceptor for AuthInterceptor where
{
    async fn intercept(&self, mut req: Request<Body>) -> Result<Request<Body>, Status> {
        match req.headers().get("authorization") {
            Some(token) => {
                let token_service = TokenService {};
                let token = token.to_str().map_err(|e| {
                    error!(target:"middleware", "Invalid user token");
                    Errors::UnAuthorized("Invalid token".to_owned())
                })?.to_owned();
                let user: User = token_service.execute(ReadFrom::AccessToken(token)).await?;
                req.extensions_mut().insert(user);
                Ok(req)
            },
            None => {
                Err(Errors::UnAuthorized("Missing authentication information".to_owned()).into())
            }
        }
    }
}
