use std::sync::Arc;

use core_services::db::{SurrealDbConnection, SurrealDbConnectionInfo};
use core_services::utils::pool_allocator::{PoolAllocator, PoolRequest};
use core_services::{s3::S3Client, smtp::client::SmtpClient};
use tokio::sync::Mutex;
use crate::repository::user::UserRepository;
use crate::services::token::TokenService;
use crate::services::user::{GenerateGuestService, SignInService, SignupService, UserService};
use crate::repository::surrealdb::user::UserRepositorySurrealDb;

pub trait DependenciesInjection {
    fn signin_service(&self) -> impl SignInService;
    fn signup_service(&self) -> impl SignupService;
    fn generate_guest_service(&self) -> impl GenerateGuestService;
    fn user_repository(&self) -> Box<dyn UserRepository>;
}

pub struct ScopedDependencies {
    pub smtp_client: SmtpClient,
    pub s3: S3Client,
    pub db_pool: Arc<Mutex<PoolAllocator<SurrealDbConnection, SurrealDbConnectionInfo>>>,
    pub user_repository: Box<dyn UserRepository + Send + Sync>

}

pub struct DevlogSdk {
    app_scoped: ScopedDependencies
}

impl DevlogSdk {
    pub fn new(smtp_client: &SmtpClient, s3: &S3Client, db_pool: Arc<Mutex<PoolAllocator<SurrealDbConnection, SurrealDbConnectionInfo>>>) -> Self {
        let db_request = PoolRequest {
            pool: db_pool.clone()
        };

        Self {
            app_scoped: ScopedDependencies {
                user_repository: Box::new(UserRepositorySurrealDb {db: db_request.clone()}),
                smtp_client: smtp_client.clone(),
                s3: s3.clone(),
                db_pool
            }
        }
    }

    fn new_user_service(&self) -> UserService {
        UserService {
            user_repository: &self.app_scoped.user_repository,
            token_service: self.new_token_service(),
            s3: &self.app_scoped.s3,
            smtp_client: &self.app_scoped.smtp_client,
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
            db: PoolRequest { pool: self.app_scoped.db_pool.clone() }
       })
    }
}
