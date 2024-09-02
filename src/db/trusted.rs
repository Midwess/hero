use schema::misc::datetime::Datetime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::ops::{Deref, DerefMut};
use std::vec;
use surreal_devl::serialize::SurrealSerialize;
use surreal_devl::wrapper::SurrealQR;
use surrealdb::method::Stats;
use surrealdb::opt::{QueryResult, RecordId};
use surrealdb::Response as QueryResponse;

#[derive(Clone, Serialize, Deserialize,)]
#[serde(untagged)]
pub enum DbIntent<T,>
where
    T: SurrealSerialize + Clone,
{
    New(T,),
    Update(T,),
    SoftDelete(T,),
}

impl<T,> Deref for DbIntent<T,>
where
    T: SurrealSerialize,
{
    type Target = T;

    fn deref(&self,) -> &Self::Target {
        match self {
            DbIntent::New(data,) => data,
            DbIntent::Update(data,) => data,
            DbIntent::SoftDelete(data,) => data,
        }
    }
}

impl<T,> SurrealSerialize for DbIntent<T,>
where
    T: SurrealSerialize + Clone,
{
    fn into_idiom_value(&self,) -> Vec<(surrealdb::sql::Idiom, surrealdb::sql::Value,),> {
        match self {
            DbIntent::New(data,) => {
                let mut idioms = data.into_idiom_value();
                let mut action_idioms = OneState::new_now().into_idiom_value();
                idioms.append(&mut action_idioms,);
                idioms
            }
            DbIntent::Update(data,) => {
                let mut idioms = data.into_idiom_value();
                let mut action_idioms = OneState::update_now().into_idiom_value();
                idioms.append(&mut action_idioms,);
                idioms
            }
            DbIntent::SoftDelete(data,) => {
                let mut idioms = data.into_idiom_value();
                let mut action_idioms = OneState::delete_now().into_idiom_value();
                idioms.append(&mut action_idioms,);
                idioms
            }
        }
    }
}

impl<T,> From<DbIntent<T,>,> for RecordId
where
    T: SurrealSerialize,
{
    fn from(_: DbIntent<T,>,) -> Self {
        panic!("The DbIntent can not be use directly but must use as part of the content");
    }
}

#[derive(Clone, Serialize, Deserialize, surreal_derive_plus::SurrealDerive,)]
pub struct OneState {
    pub created_at: Option<Datetime,>,
    pub updated_at: Option<Datetime,>,
    pub deleted_at: Option<Datetime,>,
}

impl From<OneState,> for RecordId {
    fn from(_: OneState,) -> Self {
        panic!("The OneState is not able to serialize into database alone, instead use it with other type");
    }
}

impl From<Value,> for OneState {
    fn from(value: Value,) -> Self {
        value.into()
    }
}

impl OneState {
    pub fn new(time: Datetime,) -> Self {
        Self {
            created_at: Some(time,),
            updated_at: None,
            deleted_at: None,
        }
    }

    pub fn new_now() -> Self {
        Self {
            created_at: Some(Datetime::now(),),
            updated_at: None,
            deleted_at: None,
        }
    }

    pub fn update_now() -> Self {
        Self {
            created_at: None,
            updated_at: Some(Datetime::now(),),
            deleted_at: None,
        }
    }

    pub fn delete_now() -> Self {
        Self {
            created_at: None,
            updated_at: None,
            deleted_at: Some(Datetime::now(),),
        }
    }
}

// The option by default can not be null
// it only be null in case we take it out of the wrapper TrustedOne
#[derive(Serialize, Deserialize,)]
pub struct TrustedOne(SurrealQR,);

impl TrustedOne {
    pub fn none() -> Self {
        Self(SurrealQR(surrealdb::sql::Value::None,),)
    }
}

impl Into<SurrealQR,> for TrustedOne {
    fn into(self,) -> SurrealQR {
        self.0
    }
}

impl Into<surrealdb::sql::Value,> for TrustedOne {
    fn into(self,) -> surrealdb::sql::Value {
        let value: SurrealQR = self.into();
        value.into()
    }
}

impl Deref for TrustedOne {
    type Target = SurrealQR;

    fn deref(&self,) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TrustedOne {
    fn deref_mut(&mut self,) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T,> From<TrustedOne,> for Vec<T,>
where
    T: From<surrealdb::sql::Value,>,
{
    fn from(value: TrustedOne,) -> Self {
        let value: SurrealQR = value.into();
        value.into()
    }
}

impl<T,> From<TrustedOne,> for Option<T,>
where
    T: From<surrealdb::sql::Value,>,
{
    fn from(value: TrustedOne,) -> Self {
        let value: SurrealQR = value.into();
        value.into()
    }
}

impl QueryResult<TrustedOne,> for usize {
    fn query_result(self, response: &mut QueryResponse,) -> surrealdb::Result<TrustedOne,> {
        let mut value = response.take::<surrealdb::sql::Value>(self,)?;

        if value.is_none() || value.is_null() {
            return surrealdb::Result::Ok(TrustedOne::none(),)
        }

        let mut stack = vec![&mut value];

        while !stack.is_empty() {
            let value = stack.pop().unwrap();
            if let surrealdb::sql::Value::Object(surrealdb::sql::Object(ref mut entries,),) = value {
                let mut removed_keys: Vec<String,> = vec![];
                let mut object_keys: Vec<String,> = vec![];
                for (key, sub_value,) in entries.iter() {
                    if let surrealdb::sql::Value::Object(sub_entries,) = sub_value {
                        if sub_entries.get("deleted_at",).is_some() {
                            removed_keys.push(key.clone(),)
                        } else {
                            object_keys.push(key.clone(),);
                        }
                    }
                }

                let mut key = removed_keys.pop();
                while key.is_some() {
                    entries.remove(key.as_ref().unwrap(),);
                    key = removed_keys.pop();
                }

                for (key, sub_value,) in entries.iter_mut() {
                    if object_keys.iter().find(|it| it.eq(&key,),).is_some() {
                        stack.push(sub_value,);
                    }
                }
            } else if let surrealdb::sql::Value::Array(surrealdb::sql::Array(ref mut array,),) = value {
                let mut removed_indices: Vec<usize,> = vec![];
                let mut object_indices: Vec<usize,> = vec![];
                for (index, sub_value,) in array.iter().enumerate() {
                    if let surrealdb::sql::Value::Object(sub_entries,) = sub_value {
                        if sub_entries.get("deleted_at",).is_some() {
                            removed_indices.push(index,);
                        } else {
                            object_indices.push(index,);
                        }
                    }
                }

                let mut key = removed_indices.pop();
                while key.is_some() {
                    array.remove(key.unwrap(),);
                    key = removed_indices.pop();
                }

                for (key, sub_value,) in array.iter_mut().enumerate() {
                    if object_indices.iter().find(|it| **it == key,).is_some() {
                        stack.push(sub_value,);
                    }
                }
            }
        }

        surrealdb::Result::Ok(TrustedOne(SurrealQR(value,),),)
    }

    fn stats(&self, _: &QueryResponse,) -> Option<Stats,> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schema::devlog::entities::User;
    use surrealdb::engine::local::{Db, Mem};
    use surrealdb::{Error as SurrealError, Surreal};

    async fn setup_db() -> Surreal<Db,> {
        let db = Surreal::new::<Mem,>((),).await.unwrap();

        db.use_ns("test",).use_db("test",).await.unwrap();

        // Insert test data into the database
        db.query("CREATE user SET name = 'John Doe', password = '123', email = 'email';",).await.unwrap();
        db.query("CREATE user SET name = 'Jane Doe', password = '123', email = 'email', deleted_at=time::now()",)
            .await
            .unwrap();

        db
    }

    #[tokio::test]
    async fn test_query_result_with_valid_response() -> Result<(), SurrealError,> {
        let db = setup_db().await;

        let response: TrustedOne =
            db.query("SELECT * FROM user WHERE name = 'John Doe';",).await.unwrap().take(0,).unwrap();
        let query_result: Option<User,> = response.into();
        assert_eq!(
            query_result,
            Some(User {
                name: "John Doe".to_string(),
                password: "123".to_string(),
                email: "email".to_string(),
                avatar_object: None
            })
        );

        Ok((),)
    }

    #[tokio::test]
    async fn test_query_result_with_soft_deleted_data() -> Result<(), SurrealError,> {
        let db = setup_db().await;

        let response: TrustedOne =
            db.query("SELECT * FROM user WHERE name = 'Jane Doe';",).await.unwrap().take(0,).unwrap();
        let mut response: Option<User,> = response.into();
        println!("Found a value = {:?}", response.take());
        assert_eq!(response.take().is_some(), false);
        Ok((),)
    }

    #[tokio::test]
    async fn test_query_result_with_vec() -> Result<(), SurrealError,> {
        let db = setup_db().await;

        let response: TrustedOne = db.query("SELECT * FROM user",).await.unwrap().take(0,).unwrap();
        let response: Vec<User,> = response.try_into().unwrap();
        assert_eq!(
            response,
            vec![User {
                name: "John Doe".to_string(),
                password: "123".to_string(),
                email: "email".to_string(),
                avatar_object: None
            }]
        );
        Ok((),)
    }

    #[tokio::test]
    async fn test_query_result_with_empty_response() -> Result<(), SurrealError,> {
        let db = setup_db().await;

        let response: TrustedOne =
            db.query("SELECT * FROM user WHERE name = 'Non Existent';",).await.unwrap().take(0,).unwrap();
        let query_result: Option<User,> = response.into();
        assert_eq!(query_result.is_none(), true);

        Ok((),)
    }

    #[tokio::test]
    async fn test_query_result_with_multiple_valid_results() -> Result<(), SurrealError,> {
        let db = setup_db().await;

        // Insert another valid user
        db.query("CREATE user SET name = 'John Smith', password = '456', email = 'john.smith@example.com';",)
            .await
            .unwrap();

        let response: TrustedOne = db.query("SELECT * FROM user",).await.unwrap().take(0,).unwrap();
        let response: Vec<User,> = response.try_into().unwrap();

        // Ensure both users are returned in the correct order
        assert_eq!(response.len(), 2);
        assert!(response.contains(&User {
            name: "John Doe".to_string(),
            password: "123".to_string(),
            email: "email".to_string(),
            avatar_object: None
        }));
        assert!(response.contains(&User {
            name: "John Smith".to_string(),
            password: "456".to_string(),
            email: "john.smith@example.com".to_string(),
            avatar_object: None
        }));

        Ok((),)
    }

    #[tokio::test]
    async fn test_query_result_with_mixed_soft_deleted_data() -> Result<(), SurrealError,> {
        let db = setup_db().await;

        // Insert an additional soft-deleted user
        db.query(
            "CREATE user SET name = 'Soft Deleted User', password = '123', email = 'email', deleted_at = time::now();",
        )
        .await
        .unwrap();

        let response: TrustedOne = db.query("SELECT * FROM user",).await.unwrap().take(0,).unwrap();
        let response: Vec<User,> = response.into();

        // Ensure that only the non-soft-deleted users are returned
        assert_eq!(response.len(), 1);
        assert!(response.contains(&User {
            name: "John Doe".to_string(),
            password: "123".to_string(),
            email: "email".to_string(),
            avatar_object: None
        }));

        Ok((),)
    }

    #[tokio::test]
    async fn test_query_result_with_only_soft_deleted_data() -> Result<(), SurrealError,> {
        let db = setup_db().await;

        // Insert a soft-deleted user and remove the non-deleted one
        db.query("DELETE user WHERE name = 'John Doe';",).await.unwrap();
        db.query(
            r#"
            CREATE user
            SET name = 'Soft Deleted Only User', password = '123', email = 'email', deleted_at = time::now();
        "#,
        )
        .await
        .unwrap();

        let response: TrustedOne = db.query("SELECT * FROM user",).await.unwrap().take(0,).unwrap();
        let response: Vec<User,> = response.into();

        // Ensure that the result is empty because all users are soft-deleted
        assert_eq!(response.len(), 0);

        Ok((),)
    }
}
