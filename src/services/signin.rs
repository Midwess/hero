use crate::{services::{base::{Resolve, Service}, errors::Errors, token::{GenerateBy, TokenService}}, Db};
use schema::devlog::entities::{signin_method, SigninMethod, Token, User, UserId};
use surreal_derive_plus::surreal_quote;
use crate::config::CONFIGS;

pub struct SigninService {
    pub db: Db,
    pub token_service: TokenService
}

impl<'a> Service<&'a SigninMethod, Token> for SigninService {
    async fn execute(self, params: &'a SigninMethod) -> Resolve<Token> {
        let method = params.method.as_ref().unwrap();
        match method {
            signin_method::Method::ByEmailPassword(value) => {
                let user_id = UserId { email: value.email.clone() };
                let user: Option<User> = self.db.query(surreal_quote!("SELECT * FROM #val(&user_id)")).await?.take(0)?;
                if user.is_none() || user.as_ref().unwrap().password != value.password {
                    return Err(Errors::UnAuthorized(format!("Incorrect user name or password {}", value.email.as_str()).to_owned()))
                }

                let token = self.token_service.execute(GenerateBy::User(user.unwrap(), CONFIGS.token.access_token_duration)).await?;

                Ok(token)
            },
            _ => Err(Errors::UnAuthorized("".to_owned()))
        }
    }
}
