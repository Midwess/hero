use std::time::Duration;

use crate::{s3::S3Client, services::{base::{Resolve, Service}, errors::Errors, token::{GenerateBy, TokenService}}, Db};
use schema::devlog::entities::{Token, User, UserId};
use schema::misc::authentication::{sign_in_method, sign_up_method, SignInMethod, SignUpMethod};
use surreal_derive_plus::surreal_quote;
use surreal_devl::wrapper::SurrealQR;
use crate::config::CONFIGS;

pub struct SigninService {
    pub db: Db,
    pub token_service: TokenService,
    pub s3: S3Client
}

#[derive(Debug, Clone)]
pub struct SigninResult {
    pub user: User,
    pub token: Token
}

impl<'a> Service<&'a SignInMethod, SigninResult> for SigninService {
    async fn execute(self, params: &'a SignInMethod) -> Resolve<SigninResult> {
        let method = params.method.as_ref().unwrap();
        match method {
            sign_in_method::Method::ByEmailPassword(value) => {
                if value.email.is_empty() || value.password.is_empty() {
                    return Err(Errors::BadRequest("Email and password can not be empty".to_owned()))
                }

                let user_id = UserId { email: value.email.clone() };
                let user: SurrealQR = self.db.query(surreal_quote!("SELECT * FROM #val(&user_id)")).await?.take(0)?;
                let mut user = user.object()?.map(|it| User::from(it));
                if user.is_none() || user.as_ref().unwrap().password != value.password {
                    return Err(Errors::UnAuthorized(format!("Incorrect user name or password {}", value.email.as_str()).to_owned()))
                }

                let mut user = user.take().unwrap();
                self.s3.sign_object(user.avatar_object.as_mut().unwrap(), Duration::from_secs(60 * 60 * 24 * 7)).await?;
                let token = self.token_service.execute(GenerateBy::User(&user, CONFIGS.token.access_token_duration)).await?;
                let result = SigninResult {
                    user,
                    token,
                };

                Ok(result)
            },
            _ => Err(Errors::UnAuthorized("".to_owned()))
        }
    }
}
