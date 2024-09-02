use core_services::services::base::{Resolve, Service};
use core_services::services::errors::Errors;

use crate::config::CONFIGS;
use async_trait::async_trait;
use schema::devlog::entities::UserId;
use schema::misc::authentication::{sign_in_method, SignInMethod};

use crate::services::token::GenerateBy;
use crate::services::user::SigninResult;

use super::UserService;

#[async_trait]
impl<'a> Service<&'a SignInMethod, SigninResult> for UserService {
    async fn execute(&self, params: &'a SignInMethod) -> Resolve<SigninResult> {
        let method = params.method.as_ref().unwrap();
        match method {
            sign_in_method::Method::ByEmailPassword(value) => {
                if value.email.is_empty() || value.password.is_empty() {
                    return Err(Errors::BadRequest("Email and password can not be empty".to_owned()))
                }

                let user_id = UserId {
                    email: value.email.clone()
                };
                let mut user =
                    self.user_repository.find_one(crate::repository::user::UserFind::UserId(&user_id)).await?;
                if user.is_none() || user.as_ref().unwrap().password != value.password {
                    return Err(Errors::UnAuthorized(
                        format!("Incorrect user name or password {}", value.email.as_str()).to_owned()
                    ))
                }

                let mut user = user.take().unwrap();
                self.s3.use_cdn(user.avatar_object.as_mut().unwrap()).await?;
                let token =
                    self.token_service.execute(GenerateBy::User(&user, CONFIGS.token.access_token_duration)).await?;
                let result = SigninResult { user, token };

                Ok(result)
            }
            _ => Err(Errors::BadRequest("Unlupported Signin method".to_owned()))
        }
    }
}
