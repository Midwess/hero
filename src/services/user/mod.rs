pub mod guest;
pub mod signin;
pub mod signup;

use core_services::s3::S3Client;
use core_services::services::base::Service;
use core_services::smtp::client::SmtpClient;
use core_services::utils::pool::request::PoolRequest;
use schema::devlog::entities::{Token, User};
use schema::misc::authentication::{SignInMethod, SignUpMethod};

use crate::repository::user::UserRepository;

use super::token::TokenService;

pub struct UserService {
    pub user_repository: Box<dyn UserRepository + Send + Sync + 'static>,
    pub s3: S3Client,
    pub smtp_client: SmtpClient,
    pub token_service: TokenService
}

#[derive(Debug, Clone)]
pub enum GenerateGuestParams {
    ToFullFill()
}

pub trait GenerateGuestService: Service<GenerateGuestParams, User> {}

impl GenerateGuestService for UserService {}

#[derive(Debug, Clone)]
pub struct SigninResult {
    pub user: User,
    pub token: Token
}

pub trait SignInService<'a>: Service<&'a SignInMethod, SigninResult> {}

impl<'a> SignInService<'a> for UserService {}

#[derive(Debug, Clone)]
pub struct SignupResult {
    pub token: Token,
    pub user: User
}

pub trait SignupService<'a>: Service<&'a SignUpMethod, SignupResult> {}

impl<'a> SignupService<'a> for UserService {}
