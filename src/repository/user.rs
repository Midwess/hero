use async_trait::async_trait;
use core_services::services::base::Resolve;
use schema::devlog::entities::{User, UserId};

pub enum UserFind<'a,> {
    UserId(&'a UserId,),
}

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn create(&self, user: User,) -> Resolve<User,>;

    async fn find_one<'a,>(&self, finding: UserFind<'a,>,) -> Resolve<Option<User,>,>;
}

pub struct UserRepositoryImpl {}
