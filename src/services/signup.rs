use crate::services::{base::{Resolve, Service}, errors::Errors, token::{GenerateBy, TokenService}};
use crate::config::CONFIGS;
use schema::devlog::entities::{signup_method, SignupMethod, Token, User, UserId};
use surreal_derive_plus::surreal_quote;

use crate::Db;

pub struct SignupService {
    pub db: Db,
    pub token_service: TokenService
}

#[derive(Debug, Clone)]
pub struct SignupResult {
    pub token: Token,
    pub user: User
}

impl<'a> Service<&'a SignupMethod, SignupResult> for SignupService {
    async fn execute(self, params: &'a SignupMethod) -> Resolve<SignupResult> {
        let email: Option<&str> = match params.method.as_ref().unwrap() {
            signup_method::Method::ByEmail(value) => Some(value.email.as_str()),
            signup_method::Method::ByEmailPassword(value) => Some(value.email.as_str()),
            _ => None
        };

        let existing_user: Option<User> = match email {
            Some(email) => {
                let user_id = UserId {email: email.to_owned()};
                self.db.query(surreal_quote!("SELECT * FROM #val(&user_id)")).await?.take(0)?
            }
            None => None
        };

        if existing_user.is_some() {
            return Err(Errors::UserAlreadyExist(email.unwrap().to_owned()));
        }

        let new_user = match params.method.as_ref().unwrap() {
            signup_method::Method::ByEmail(value) => {
                User { name: "".to_owned(), email: value.email.clone(), password: "".to_owned() }
            },
            signup_method::Method::ByEmailPassword(value) => {
                User { name: value.display_name.to_owned(), email: value.email.clone(), password: value.password.clone() }
            },
            signup_method::Method::ByDisplayName(value) => {
                // This is all about the business :((
                let email = format!("{}@no-email.com", value.display_name.as_str());
                User { name: value.display_name.clone(), email, password: "".to_owned() }
            },
        };

        let mut created_user: Option<User> = self.db.query(surreal_quote!("CREATE #record(&new_user)")).await?.take(0)?;

        let token = self.token_service.execute(GenerateBy::User(
            created_user.as_ref().expect("Not able to create user"),
            CONFIGS.token.access_token_duration)).await?;

        let result = SignupResult {
            token,
            user: created_user.take().unwrap(),
        };

        Ok(result)
    }
}
