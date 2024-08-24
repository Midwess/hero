use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use core_services::services::{base::{self, Resolve, Service}, errors::Errors};
use schema::{devlog::entities::{Token, User}, misc::datetime::Datetime};
use core_services::config::CONFIGS;

#[derive(Debug, Clone)]
pub struct TokenService {}

#[derive(Clone, Debug)]
pub enum GenerateBy<'a> {
    User(&'a User, Duration)
}

#[derive(Clone, Debug)]
pub enum ReadFrom {
    AccessToken(String)
}


#[derive(Deserialize, Serialize)]
struct Claims {
    exp: usize,
    other: HashMap<String, String>,
}

impl TokenService {
    fn read_claims_from_jwt(&self, token: &str, secret: &str) -> Resolve<Vec<(String, String)>> {
        let mut validation = Validation::default();
        validation.validate_exp = true;


        let token_data: TokenData<Claims> = decode::<Claims>(token, &DecodingKey::from_secret(secret.as_ref()), &validation)?;

        let claims = token_data.claims.other;
        let result: Vec<(String, String)> = claims.into_iter().collect();

        Ok(result)
    }

    pub fn generate_jwt(&self, claims: Vec<(&str, &str)>, duration: Duration, secret: String) -> Result<Token, Errors> {
        let expiration = chrono::Utc::now() + duration;
        let exp = expiration.timestamp() as usize;

        let mut other_claims = HashMap::new();
        for (key, value) in claims {
            other_claims.insert(key.to_string(), value.to_string());
        }

        let claims = Claims { exp, other: other_claims };

        let token = encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_ref()))?;

        Ok(Token {
            content: token,
            expired_at: Some(Datetime {
                utc_millis_since_epoch: exp as u64,
            })
        })
    }
}

impl<'a> Service<GenerateBy<'a>, Token> for TokenService {
    async fn execute(self, params: GenerateBy<'a>) -> base::Resolve<Token> {
        match params {
            GenerateBy::User(user, duration) => {
                let claims = vec![
                    ("name", user.name.as_str()),
                    ("email", user.email.as_str()),
                ];

                self.generate_jwt(
                    claims,
                    duration,
                    CONFIGS.token.jwt_access_token_private_key.clone()
                )
            }
            _ => Err(Errors::UnAuthorized("".to_owned()))
        }
    }
}

impl Service<ReadFrom, User> for TokenService {
    async fn execute(self, params: ReadFrom) -> base::Resolve<User> {

        match params {
            ReadFrom::AccessToken(token) => {
                let claims = self.read_claims_from_jwt(
                    token.as_str(), CONFIGS.token.jwt_access_token_private_key.as_str())?;
                let mut user = User::default();

                for (key, value) in claims.iter() {
                    match key.as_str() {
                        "name" => user.name = value.to_owned(),
                        "email" => user.email = value.to_owned(),
                        _ => {}
                    }
                }

                Ok(user)
            }
        }
    }
}
