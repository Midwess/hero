use std::time::Duration;

use schema::devlog::entities::User;
use surrealdb::sql::Uuid;

use core_services::config::CONFIGS;
use core_services::services::base::{Resolve, Service};
use core_services::utils::random_number_in_range;

use super::{GenerateGuestParams, UserService};

#[async_trait::async_trait]
impl<'a,> Service<GenerateGuestParams, User,> for UserService {
    async fn execute(&self, params: GenerateGuestParams,) -> Resolve<User,> {
        let mut default_avatar_path = self.s3.get_default_avatars_object().await?;
        let random_index = random_number_in_range(0, default_avatar_path.iter().count() as u32,);
        let random_avatar = default_avatar_path.remove(random_index as usize,);
        match params {
            GenerateGuestParams::ToFullFill() => {
                let claims = vec![("isGuest", "true",)];

                let token = self.token_service.generate_jwt(
                    claims,
                    Duration::from_mins(1,),
                    CONFIGS.token.jwt_access_token_private_key.clone(),
                )?;

                let id = Uuid::new_v4();
                return Ok(User {
                    password: token.content,
                    name: format!("guest-{}", id.0.to_string()),
                    email: format!("guest-{}@devlog.studio", id.0.to_string()),
                    avatar_object: Some(random_avatar,),
                },)
            }
        }
    }
}
