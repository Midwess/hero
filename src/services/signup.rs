use std::time::Duration;
use surreal_devl::wrapper::SurrealQR;
use crate::{s3::S3Client, services::{base::{Resolve, Service}, errors::Errors, token::{GenerateBy, TokenService}}, utils::random_number_in_range};
use crate::config::CONFIGS;
use surreal_derive_plus::surreal_quote;
use surrealdb::sql::Object;
use schema::misc::authentication::{sign_up_method, SignUpMethod};
use schema::devlog::entities::{Token, User, UserId};

use crate::Db;

pub struct SignupService {
    pub db: Db,
    pub token_service: TokenService,
    pub s3: S3Client
}

#[derive(Debug, Clone)]
pub struct SignupResult {
    pub token: Token,
    pub user: User
}

impl<'a> Service<&'a SignUpMethod, SignupResult> for SignupService {
    async fn execute(self, params: &'a SignUpMethod) -> Resolve<SignupResult> {
        let email: Option<&str> = match params.method.as_ref().unwrap() {
            sign_up_method::Method::ByEmail(value) => Some(value.email.as_str()),
            sign_up_method::Method::ByEmailPassword(value) => Some(value.email.as_str()),
            _ => None
        };

        match email {
            Some(email) => {
                let user_id = UserId {email: email.to_owned()};
                let value: SurrealQR = self.db.query(surreal_quote!("SELECT * FROM #val(&user_id)")).await?.take(0)?;
                let value = value.object()?;
                if value.is_some() {
                    return Err(Errors::UnAuthorized("Already exist".to_owned()));
                }
            }
            None => {}
        };

        let mut current_default_avatars = self.s3.get_default_avatars_object().await?;
        let random_index = random_number_in_range(0, current_default_avatars.len() as u32 - 1);
        let mut random_avatar = current_default_avatars.get(random_index as usize).take().unwrap().to_owned();

        let new_user = match params.method.as_ref().unwrap() {
            sign_up_method::Method::ByEmail(value) => {
                User {
                    avatar_object: Some(random_avatar.clone()),
                    name: "".to_owned(),
                    email: value.email.clone(),
                    password: "".to_owned()
                }
            },
            sign_up_method::Method::ByEmailPassword(value) => {
                User { avatar_object: Some(random_avatar.clone()), name: "".to_owned(), email: value.email.clone(), password: value.password.clone() }
            },
            sign_up_method::Method::ByDisplayName(value) => {
                // This is all about the business :((
                let email = format!("{}@no-email.com", value.display_name.as_str());
                User { avatar_object: Some(random_avatar.clone()), name: value.display_name.clone(), email, password: "".to_owned() }
            },
        };

        let created_user: SurrealQR = self.db.query(surreal_quote!("CREATE #record(&new_user)")).await?.take(0)?;
        let created_user = created_user.object()?;
        let mut created_user: User = created_user.map(|it| it.into()).expect("Error while creating user");

        let token = self.token_service.execute(GenerateBy::User(
            &created_user,
            CONFIGS.token.access_token_duration)).await?;

        self.s3.sign_object(&mut random_avatar, Duration::from_days(7)).await?;
        created_user.avatar_object = Some(random_avatar);

        let result = SignupResult {
            token,
            user: created_user,
        };

        Ok(result)
    }
}
