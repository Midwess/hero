use core_services::db::{SurrealDbConnection, SurrealDbConnectionInfo};
use core_services::utils::pool::request::PoolRequest;
use core_services::{s3::S3Client, smtp::client::SmtpClient};
use lettre::{SmtpTransport, Transport};
use crate::grpc::middleware::auth::AuthInterceptor;
use crate::repository::surrealdb::user::UserRepositorySurrealDb;
use crate::repository::user::UserRepository;
use crate::services::token::TokenService;
use crate::services::user::{GenerateGuestService, SignInService, SignupService, UserService};

pub trait DependenciesInjection {
    fn signin_service(&self) -> impl SignInService;
    fn signup_service(&self) -> impl SignupService;
    fn generate_guest_service(&self) -> impl GenerateGuestService;
    fn user_repository(&self) -> Box<dyn UserRepository>;
    fn s3_client(&self) -> S3Client;
    fn smtp_client(&self) -> SmtpClient;
    fn get_grpc_middleware_auth(&'static self) -> AuthInterceptor;
}

pub struct SharingResource {
    pub smtp_transport: PoolRequest<SmtpTransport, ()>,
    pub s3: PoolRequest<aws_sdk_s3::client::Client, ()>,
    pub devlog_db: PoolRequest<SurrealDbConnection, SurrealDbConnectionInfo> 
}

pub struct DevlogSdk {
    sharing_resource: SharingResource
}

impl DevlogSdk {
    pub fn new(sharing_resource: SharingResource) -> Self {
        Self {
            sharing_resource
        }
    }

    fn new_user_service(&self) -> UserService {
        UserService {
            user_repository: self.user_repository(),
            token_service: self.new_token_service(),
            s3: self.s3_client(),
            smtp_client: self.smtp_client(),
        }
    } 

    fn new_token_service(&self) -> TokenService {
        TokenService {}
    }
}

impl DependenciesInjection for DevlogSdk {
    fn signin_service(&self) -> impl SignInService {
        self.new_user_service()
    }

    fn signup_service(&self) -> impl SignupService {
        self.new_user_service()
    }

    fn generate_guest_service(&self) -> impl GenerateGuestService {
        self.new_user_service()
    }

    fn user_repository(&self) -> Box<dyn UserRepository> {
        Box::new(UserRepositorySurrealDb {
            db: self.sharing_resource.devlog_db.clone()
        })
    }

    fn s3_client(&self) -> S3Client {
        S3Client {
            client: self.sharing_resource.s3.clone()
        }
    }

    fn smtp_client(&self) -> SmtpClient {
        SmtpClient {
            transport: self.sharing_resource.smtp_transport.clone(),
        }
    }

    fn get_grpc_middleware_auth(&'static self) -> AuthInterceptor {
        AuthInterceptor::new(self)
    }
}

