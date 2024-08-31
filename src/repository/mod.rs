use std::ops::Deref;
use schema::misc::datetime::Datetime;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use surreal_devl::serialize::SurrealSerialize;
use surreal_devl::wrapper::SurrealQR;
use surrealdb::method::Stats;
use surrealdb::opt::{QueryResult, RecordId};
use surrealdb::Response as QueryResponse;

/**
 * Update the current data to add some extra fields
 * that is needed to save into the database.
 * to tracking the created object is new or updated or soft deleted
 */
#[derive(Clone)]
pub struct IdiomsW(Vec<(surrealdb::sql::Idiom, surrealdb::sql::Value)>);

impl SurrealSerialize for IdiomsW {
    fn into_idiom_value(&self) -> Vec<(surrealdb::sql::Idiom, surrealdb::sql::Value)> {
        self.0.clone()
    }
}

impl From<IdiomsW> for RecordId {
    fn from(_: IdiomsW) -> Self {
        panic!("The IdiomsW can not be use to insert into db")
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DbIntent<T> where T: SurrealSerialize + Clone {
    New(T),
    Update(T),
    SoftDelete(T)
}

impl<T> Deref for DbIntent<T> where T: SurrealSerialize {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            DbIntent::New(data) => data,
            DbIntent::Update(data) => data,
            DbIntent::SoftDelete(data) => data,
        }
    }
}

impl<T> SurrealSerialize for DbIntent<T> where T: SurrealSerialize + Clone {
    fn into_idiom_value(&self) -> Vec<(surrealdb::sql::Idiom, surrealdb::sql::Value)> {
        match self {
            DbIntent::New(data) => {
                let mut idioms = data.into_idiom_value();
                let mut action_idioms = OneState::new_now().into_idiom_value();
                idioms.append(&mut action_idioms);
                idioms
            }
            DbIntent::Update(data) => {
                let mut idioms = data.into_idiom_value();
                let mut action_idioms = OneState::update_now().into_idiom_value();
                idioms.append(&mut action_idioms);
                idioms
            }
            DbIntent::SoftDelete(data) => {
                let mut idioms = data.into_idiom_value();
                let mut action_idioms = OneState::delete_now().into_idiom_value();
                idioms.append(&mut action_idioms);
                idioms
            }
        }
    }
}

impl<T> From<DbIntent<T>> for RecordId where T: SurrealSerialize {
    fn from(_: DbIntent<T>) -> Self {
        panic!("The DbIntent can not be use directly but must use as part of the content");
    }
}

#[derive(Clone, Serialize, Deserialize, surreal_derive_plus::SurrealDerive)]
pub struct OneState {
    pub created_at: Option<Datetime>,
    pub updated_at: Option<Datetime>,
    pub deleted_at: Option<Datetime>
}

impl From<OneState> for RecordId {
    fn from(_: OneState) -> Self {
        panic!("The OneState is not able to serialize into database alone, instead use it with other type");
    }
}

impl From<Value> for OneState {
    fn from(value: Value) -> Self {
        value.into()
    }
}

impl OneState {
    pub fn new(time: Datetime) -> Self {
        Self {
            created_at: Some(time),
            updated_at: None,
            deleted_at: None
        }
    }

    pub fn new_now() -> Self {
        Self {
            created_at: Some(Datetime::now()),
            updated_at: None,
            deleted_at: None
        }
    }

    pub fn update_now() -> Self {
        Self {
            created_at: None,
            updated_at: Some(Datetime::now()),
            deleted_at: None
        }
    }

    pub fn delete_now() -> Self {
        Self {
            created_at: None,
            updated_at: None,
            deleted_at: Some(Datetime::now())
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum TrustedOne<T> {
    Exist(T, Option<OneState>),
    SoftDeleted(T, OneState),
    None,
}

impl<T: DeserializeOwned> QueryResult<TrustedOne<T>> for usize {
    fn query_result(self, response: &mut QueryResponse) -> surrealdb::Result<TrustedOne<T>> {
        let mut value = response.take::<surrealdb::sql::Value>(0)?;

        if value.is_none() || value.is_null() {
            return surrealdb::Result::Ok(TrustedOne::None)
        }
        
        let mut stack = vec![&mut value];

        while !stack.is_empty() {
            let value = stack.pop().unwrap();
            if let surrealdb::sql::Value::Object(surrealdb::sql::Object(ref mut entries)) = value {
                let mut removed_keys: Vec<String> = vec![];
                let mut object_keys: Vec<String> = vec![];
                for (key, sub_value) in entries.iter() {
                    if let surrealdb::sql::Value::Object(sub_entries) = sub_value {
                        if sub_entries.get("deleted_at").is_some() {
                            removed_keys.push(key.clone())
                        }
                        else {
                            object_keys.push(key.clone());
                        }
                    }
                }

                let mut key = removed_keys.pop();
                while key.is_some() {
                    key = removed_keys.pop();
                }

                for (key, sub_value) in entries.iter_mut() {
                    if object_keys.iter().find(|it| it.eq(&key)).is_some() {
                        stack.push(sub_value);
                    }
                }
            }
            else if let surrealdb::sql::Value::Array(surrealdb::sql::Array(ref mut array)) = value {
                let mut removed_indices: Vec<usize> = vec![];
                let mut object_indices: Vec<usize> = vec![];
                for (index, sub_value) in array.iter().enumerate() {
                    if let surrealdb::sql::Value::Object(sub_entries) = sub_value {
                        if sub_entries.get("deleted_at").is_some() {
                            removed_indices.push(index);
                        }
                        else {
                            object_indices.push(index);
                        }
                    }
                }

                let mut key = removed_indices.pop();
                while key.is_some() {
                    std::mem::swap(&mut array[key.unwrap()], &mut surrealdb::sql::Value::None);
                    key = removed_indices.pop();
                }

                for (key, sub_value) in array.iter_mut().enumerate() {
                    if object_indices.iter().find(|it| **it == key).is_some() {
                        stack.push(sub_value);
                    }
                }
            }
        }

        surrealdb::Result::Ok(TrustedOne::None)
    }

    fn stats(&self, _: &QueryResponse) -> Option<Stats> {
        None
    }
}

#[cfg(test)]
pub mod test {
    use serde::{Deserialize, Serialize};
    use tokio::test;
    use crate::Db;
    use crate::repository::OneState;

    #[derive(Serialize, Deserialize)]
    struct User {
        name: String
    }

    struct Repo {
        db: Db
    }

    impl Repo {
        async fn test(&self) {
            let result: Option<User> = self.db.query("").await.unwrap().take(0).expect("");
        }
    }

    #[test]
    pub async fn do_test() {
        let user = User {
            name: String::from("Dang Minh Tien")
        };

        let one_state = OneState::new_now();

        let one = One::Exist(user, Some(one_state));
        let json = serde_json::to_string(&one).unwrap();

        println!("Json: {}", json);
    }
}

