use std::usize;

use serde::de::DeserializeOwned;
use surreal_derive_plus::surreal_quote;
use surreal_devl::proxy::default::{SurrealDeserializer, SurrealSerializer};
use surreal_devl::surreal_id::SurrealId;
use surreal_devl::surreal_qr::{RPath, SurrealQR};
use surreal_devl::surreal_statement;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;

use crate::utils::pool::reponse::PoolResponse;

use super::errors::{RepositoryError, Resolve};
use super::id::DbId;
use super::table::Table;
pub trait SendSync: Sized + Sync + Send {}

impl<T> SendSync for T where T: Sized + Sync + Send {}

#[async_trait::async_trait]
pub trait LocalSurrealDbRepository<Record, Id>
where
    Record: SurrealId + SendSync + SurrealDeserializer + SurrealSerializer + DeserializeOwned + Table<Id> + Clone,
    Id: DbId + SendSync + Clone
{
    async fn get_db(&self) -> PoolResponse<Surreal<Any>>;

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

    async fn update_or_create(&self, data: Record) -> Resolve<Record>
    where
        Record: 'async_trait
    {
        let id = Table::id(&data);
        let record = self.find_one(&id).await;
        if let Ok(Some(_record)) = record {
            self.update_one(data).await
        } else {
            self.create(data).await
        }
    }

    async fn find_one(&self, record_id: &Id) -> Resolve<Option<Record>> {
        let db = self.get_db().await;
        let table = Record::get_table().to_owned();
        let value = record_id.clone().serialize();
        let mut id_statement = "".to_owned();
        match value {
            surrealdb::sql::Value::Array(array) => {
                let mut matched_statement = 0;
                for (index, item) in array.0.iter().enumerate() {
                    let value = SurrealQR(item.clone());
                    if value.is_none() {
                        continue;
                    }

                    if matched_statement == 0 {
                        id_statement = format!("id[{index}]={}", surreal_statement::val(&value.0));
                    } else {
                        id_statement.push_str(&format!(" AND id[{index}]={}", surreal_statement::val(&value.0)));
                    }

                    matched_statement += 1;
                }
            }
            _ => {
                id_statement = format!("id={}", surreal_statement::val(&value));
            }
        }

        let result: Option<Record> = db
            .query(surreal_quote!("SELECT * FROM #table WHERE #id_statement LIMIT 1"))
            .await?
            .take(RPath::from(0))?;
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
        let record = self.find_one(record_id).await;

        let db = self.get_db().await;
        if let Ok(Some(record)) = record {
            let record_id = SurrealId::id(&record);
            db.query(surreal_quote!("DELETE #val(&record_id)")).await?;
            return Ok(record);
        } else {
            Err(RepositoryError::DbError("Record not found".to_owned()))
        }
    }
}
