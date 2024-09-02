use async_trait::async_trait;
use core_services::db::{SurrealDbConnection, SurrealDbConnectionInfo};
use core_services::services::base::Resolve;
use core_services::utils::pool::request::PoolRequest;
use schema::devlog::entities::User;
use surreal_derive_plus::surreal_quote;
use surreal_devl::wrapper::SurrealQR;

use crate::repository::user::{UserFind, UserRepository};

pub struct UserRepositorySurrealDb {
    pub db: PoolRequest<SurrealDbConnection, SurrealDbConnectionInfo>
}

#[async_trait]
impl UserRepository for UserRepositorySurrealDb {
    async fn create(&self, user: User) -> Resolve<User> {
        let created_user: SurrealQR = {
            let db = self.db.retreive().await.unwrap();
            db.query(surreal_quote!("CREATE #record(&user)")).await?.take(0)?
        };

        let created_user = created_user.object()?;
        Ok(created_user.map(|it| it.into()).expect("Error while creating user"))
    }

    async fn find_one<'a>(&self, finding: UserFind<'a>) -> Resolve<Option<User>> {
        let result = match finding {
            UserFind::UserId(by_user_id) => {
                let db = self.db.retreive().await.unwrap();
                let value: SurrealQR = db.query(surreal_quote!(r#"SELECT * FROM #val(by_user_id)"#)).await?.take(0)?;
                value.object()?
            }
        };

        Ok(result.map(|it| User::from(it)))
    }
}
