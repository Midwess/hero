use core_services::services::base::Service;
use core_services::services::errors::Errors;
use log::*;
use schema::devlog::entities::User;

use crate::sdk::{DependenciesInjection, DevlogSdk};
use crate::services::token::{ReadFrom, TokenService};
use crate::services::user::GenerateGuestParams;
use tonic::codegen::http::Request;
use tonic::transport::Body;
use tonic::{async_trait, Status};
use tonic_middleware::RequestInterceptor;

#[derive(Clone)]
pub struct AuthInterceptor<'a> {
    pub white_list: Vec<String>,
    pub sdk: &'a DevlogSdk
}

impl<'a> AuthInterceptor<'a> {
    pub fn new(sdk: &'a DevlogSdk) -> Self {
        Self {
            white_list: vec![
                String::from("/devlog.devblog.rpc.PostService/interact"),
                String::from("/devlog.devblog.rpc.PostService/get"),
                String::from("/devlog.devblog.rpc.DevblogDiscussionService/get_discussions"),
            ],
            sdk
        }
    }
}

#[async_trait]
impl<'a> RequestInterceptor for AuthInterceptor<'a> {
    async fn intercept(&self, mut req: Request<Body>) -> Result<Request<Body>, Status> {
        match req.headers().get("authorization") {
            Some(token) => {
                let token_service = TokenService {};
                let token = token
                    .to_str()
                    .map_err(|_| {
                        error!(target:"middleware", "Invalid user token");
                        Errors::UnAuthorized("Invalid token".to_owned())
                    })?
                    .to_owned();
                let user: User = token_service.execute(ReadFrom::AccessToken(token)).await?;
                req.extensions_mut().insert(user);
                Ok(req)
            }
            None => {
                let path = req.uri().clone().into_parts().path_and_query.unwrap();
                let is_allowed = self
                    .white_list
                    .iter()
                    .find(|it| {
                        return path.as_str().contains(it.to_owned());
                    })
                    .is_some();

                if !is_allowed {
                    return Err(Errors::UnAuthorized("Missing authentication information".to_owned()).into())
                }

                let service = self.sdk.generate_guest_service();
                let params = GenerateGuestParams::ToFullFill();
                let guest_user = service.execute(params).await?;
                req.extensions_mut().insert(guest_user);
                Ok(req)
            }
        }
    }
}
