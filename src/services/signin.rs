use core_services::{
    s3::S3Client,
    services::{base::{Resolve, Service},
    errors::Errors},
    Db,
};

use surreal_derive_plus::surreal_quote;
use surreal_devl::wrapper::SurrealQR;

use schema::{devlog::entities::{Token, User, UserId}, misc::authentication::SignInMethod};
use schema::misc::authentication::sign_in_method;
use crate::config::CONFIGS;

use super::token::{GenerateBy, TokenService};

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
                    return Err(Errors::UnAuthorized(
                        format!("Incorrect user name or password {}", value.email.as_str()).to_owned())
                    )
                }

                let mut user = user.take().unwrap();
                self.s3.use_cdn(user.avatar_object.as_mut().unwrap()).await?;
                let token = self.token_service.execute(
                    GenerateBy::User(&user, CONFIGS.token.access_token_duration)
                ).await?;
                let result = SigninResult {
                    user,
                    token,
                };

                Ok(result)
            },
            _ => Err(Errors::BadRequest("Unlupported Signin method".to_owned()))
        }
    }
}

