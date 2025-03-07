use std::usize;

use serde::de::DeserializeOwned;
use surreal_derive_plus::surreal_quote;
use surreal_devl::proxy::default::{SurrealDeserializer, SurrealSerializer};
use surreal_devl::surreal_id::SurrealId;
use surreal_devl::surreal_qr::{RPath, SurrealQR};
use surreal_devl::surreal_statement;

use crate::db::remote_surrealdb::SurrealDbConnection;
use crate::utils::pool::reponse::PoolResponse;

use super::errors::{RepositoryError, Resolve};
use super::id::DbId;
use super::table::Table;

pub trait SendSync: Sized + Sync + Send {}

impl<T> SendSync for T where T: Sized + Sync + Send {}

#[async_trait::async_trait]
pub trait Repository<T, I>: Send + Sync
where
    T: SendSync + Table<I>,
    I: SendSync + DbId
{
    async fn create(&self, data: T) -> Resolve<T>
    where
        T: 'async_trait;

    async fn find_one(&self, id: &I) -> Resolve<Option<T>>;

    async fn find_all(&self, from_id: Option<&I>, to_id: Option<&I>, count: Option<usize>) -> Resolve<Vec<T>>;

    async fn delete_one(&self, id: &I) -> Resolve<T>;

    async fn update_one(&self, data: T) -> Resolve<T>
    where
        T: 'async_trait;
}

#[async_trait::async_trait]
pub trait SurrealDbRepository<Record, Id>: Repository<Record, Id>
where
    Record: SurrealId + SendSync + SurrealDeserializer + SurrealSerializer + DeserializeOwned + Table<Id> + Clone,
    Id: DbId + SendSync + Clone
{
    async fn get_db(&self) -> PoolResponse<SurrealDbConnection>;

    async fn create(&self, data: Record) -> Resolve<Record>
    where
        Record: 'async_trait
    {
        let db = self.get_db().await;
        let result: Option<Record> = db.query(surreal_quote!("CREATE #record(&data)")).await?.take(RPath::from(0))?;
        match result {
            Some(record) => Ok(record),
            None => Err(RepositoryError::DbError("Failed to create record, db return None".to_owned()))
        }
    }

    async fn update_one(&self, data: Record) -> Resolve<Record>
    where
        Record: 'async_trait
    {
        let db = self.get_db().await;
        let result: Option<Record> =
            db.query(surreal_quote!("UPDATE #id(&data) #content(&data)")).await?.take(RPath::from(0))?;
        match result {
            Some(record) => Ok(record),
            None => Err(RepositoryError::DbError("Failed to update record, db return None".to_owned()))
        }
    }

    async fn find_one(&self, record_id: &Id) -> Resolve<Option<Record>> {
        let db = self.get_db().await;
        let table = Record::get_table().to_owned();
        let value = record_id.clone().serialize();
        let mut id_statement = "".to_owned();
        match value {
            surrealdb::sql::Value::Array(array) => {
                for (index, item) in array.0.iter().enumerate() {
                    let value = SurrealQR(item.clone());
                    if value.is_none() {
                        continue;
                    }

                    if index == 0 {
                        id_statement = format!("id[{index}]={}", surreal_statement::val(&value.0));
                    } else {
                        id_statement.push_str(&format!(" AND id[{index}]={}", surreal_statement::val(&value.0)));
                    }
                }
            }
            _ => {
                id_statement = format!("id={}", surreal_statement::val(&value));
            }
        }

        let result: Option<Record> =
            db.query(surreal_quote!("SELECT * FROM #table WHERE #id_statement LIMIT 1")).await?.take(RPath::from(0))?;
        Ok(result)
    }

    async fn find_all(&self, r1: Option<&Id>, r2: Option<&Id>, count: Option<usize>) -> Resolve<Vec<Record>> {
        let db = self.get_db().await;
        let table = Record::get_table();
        let from = match r1 {
            Some(it) => format!("{}..", it.clone().id(table)),
            None => format!("{table}:..")
        };

        let to = match r2 {
            Some(r2) => surreal_devl::surreal_statement::val(r2),
            None => match from.ends_with("..") {
                true => "".to_owned(),
                false => "..".to_owned()
            }
        };

        let limit = count.map(|it| format!("LIMIT {it}")).unwrap_or("".to_owned());
        let result: Vec<Record> = db.query(format!("SELECT * FROM {from}{to} {limit}")).await?.take(RPath::from(0))?;
        Ok(result)
    }

    async fn delete_one(&self, record_id: &Id) -> Resolve<Record> {
        let db = self.get_db().await;
        let table = Record::get_table();
        let record_id = record_id.clone().id(table);
        let result: Option<Record> = db.query(surreal_quote!("DELETE #val(&record_id)")).await?.take(RPath::from(0))?;
        match result {
            Some(record) => Ok(record),
            None => Err(RepositoryError::DbError("Failed to delete record, db return None".to_owned()))
        }
    }
}

#[async_trait::async_trait]
impl<X, T, I> Repository<T, I> for X
where
    X: SurrealDbRepository<T, I> + SendSync,
    T: SurrealId + SendSync + SurrealDeserializer + SurrealSerializer + DeserializeOwned + Table<I> + Clone,
    I: DbId + SendSync + Clone
{
    async fn create(&self, data: T) -> Resolve<T>
    where
        T: 'async_trait
    {
        SurrealDbRepository::create(self, data).await
    }

    async fn update_one(&self, data: T) -> Resolve<T>
    where
        T: 'async_trait
    {
        SurrealDbRepository::update_one(self, data).await
    }

    async fn find_one(&self, record_id: &I) -> Resolve<Option<T>> {
        SurrealDbRepository::find_one(self, record_id).await
    }

    async fn find_all(&self, r1: Option<&I>, r2: Option<&I>, count: Option<usize>) -> Resolve<Vec<T>> {
        SurrealDbRepository::find_all(self, r1, r2, count).await
    }

    async fn delete_one(&self, record_id: &I) -> Resolve<T> {
        SurrealDbRepository::delete_one(self, record_id).await
    }
}
