use crate::db::repository::abstraction::repository::{Repository, SendSync};
use crate::db::surrealdb::connection::SurrealDbConnection;
use crate::db::surrealdb::id::SurrealDbId;
use crate::utils::pool::reponse::PoolResponse;
use serde::de::DeserializeOwned;
use surreal_derive_plus::surreal_quote;
use surreal_devl::proxy::default::{SurrealDeserializer, SurrealSerializer};
use surreal_devl::surreal_id::SurrealId;
use surreal_devl::surreal_qr::{RPath, SurrealQR};
use surreal_devl::surreal_statement;
use surrealdb::sql::Value;

#[async_trait::async_trait]
pub trait SurrealDbRepository<Record, Id>: Repository<Record, Id>
where
    Record: SurrealId
        + SendSync
        + SurrealDeserializer
        + SurrealSerializer
        + DeserializeOwned
        + crate::db::repository::abstraction::table::Table<Id>
        + Clone,
    Id: SurrealDbId + SendSync + Clone
{
    async fn get_db(&self) -> PoolResponse<SurrealDbConnection>;

    async fn create(&self, data: Record) -> crate::db::repository::abstraction::errors::Resolve<Record>
    where
        Record: 'async_trait
    {
        let db = self.get_db().await;
        let result: Option<Record> = db.query(surreal_quote!("CREATE #record(&data)")).await?.take(RPath::from(0))?;
        match result {
            Some(record) => Ok(record),
            None => Err(crate::db::repository::abstraction::errors::RepositoryError::DbError(
                "Failed to create record, db return None".to_owned()
            ))
        }
    }

    async fn update_one(&self, data: Record) -> crate::db::repository::abstraction::errors::Resolve<Record>
    where
        Record: 'async_trait
    {
        let db = self.get_db().await;
        let result: Option<Record> =
            db.query(surreal_quote!("UPDATE #id(&data) #content(&data)")).await?.take(RPath::from(0))?;
        match result {
            Some(record) => Ok(record),
            None => Err(crate::db::repository::abstraction::errors::RepositoryError::DbError(
                "Failed to update record, db return None".to_owned()
            ))
        }
    }

    async fn find_one(&self, record_id: &Id) -> crate::db::repository::abstraction::errors::Resolve<Option<Record>> {
        SurrealDbRepository::find_one_impl(self, record_id).await
    }

    async fn find_one_impl(&self, record_id: &Id) -> crate::db::repository::abstraction::errors::Resolve<Option<Record>> {
        let db = self.get_db().await;
        let table = Record::get_table().to_owned();
        let value = record_id.clone().serialize();

        let mut from_index_has_extra_query = None;
        let range_id = match &value {
            Value::Array(array) => {
                let mut matched_statement = 0;
                let mut range_id = array.0.clone();
                let mut is_none = false;
                for (index, item) in range_id.iter_mut().enumerate() {
                    if item.is_none() {
                        is_none = true;
                        from_index_has_extra_query = Some(index);
                    }

                    if is_none {
                        *item = Value::None;
                    }
                };

                if is_none {
                    surreal_quote!(r#":#val(&range_id).."#)
                }
                else {
                    surreal_quote!(r#":#val(&range_id)"#)
                }
            },
            _ => {
                surreal_quote!(r#":#val(&value)"#)
            }
        };

        let extra_where_statement = match (from_index_has_extra_query, value) {
            (Some(none_index), Value::Array(array)) => {
                let mut matched_statement = 0;
                let mut id_statement = "".to_owned();
                for (index, item) in array.0.iter().enumerate() {
                    if (index < none_index) {
                        continue;
                    }

                    let value = SurrealQR(item.clone());
                    if value.is_none() {
                        continue;
                    }

                    if matched_statement == 0 {
                        id_statement = format!("WHERE id[{index}]={}", surreal_statement::val(&value.0));
                    } else {
                        id_statement.push_str(&format!(" AND id[{index}]={}", surreal_statement::val(&value.0)));
                    }

                    matched_statement += 1;
                }

                if matched_statement > 0 {
                    id_statement
                } else {
                    "".to_owned()
                }
            }
            _ => {
                "".to_owned()
            }
        };

        let result: Option<Record> = db
            .query(surreal_quote!("SELECT * FROM #(table)#(range_id) #extra_where_statement LIMIT 1"))
            .await?
            .take(RPath::from(0))?;
        Ok(result)
    }

    async fn find_all(
        &self,
        r1: Option<&Id>,
        r2: Option<&Id>,
        count: Option<usize>
    ) -> crate::db::repository::abstraction::errors::Resolve<Vec<Record>> {
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

    async fn delete_one(&self, record_id: &Id) -> crate::db::repository::abstraction::errors::Resolve<Record> {
        let record = SurrealDbRepository::find_one(self, record_id).await;

        let db = self.get_db().await;
        if let Ok(Some(record)) = record {
            let record_id = SurrealId::id(&record);
            db.query(surreal_quote!("DELETE #val(&record_id)")).await?;
            return Ok(record);
        } else {
            Err(crate::db::repository::abstraction::errors::RepositoryError::DbError(
                "Record not found".to_owned()
            ))
        }
    }
}

#[async_trait::async_trait]
impl<X, T, I> Repository<T, I> for X
where
    X: SurrealDbRepository<T, I> + SendSync,
    T: SurrealId
        + SendSync
        + SurrealDeserializer
        + SurrealSerializer
        + DeserializeOwned
        + crate::db::repository::abstraction::table::Table<I>
        + Clone,
    I: SurrealDbId + SendSync + Clone
{
    async fn create(&self, data: T) -> crate::db::repository::abstraction::errors::Resolve<T>
    where
        T: 'async_trait
    {
        SurrealDbRepository::create(self, data).await
    }

    async fn update_one(&self, data: T) -> crate::db::repository::abstraction::errors::Resolve<T>
    where
        T: 'async_trait
    {
        SurrealDbRepository::update_one(self, data).await
    }

    async fn find_one(&self, record_id: &I) -> crate::db::repository::abstraction::errors::Resolve<Option<T>> {
        SurrealDbRepository::find_one(self, record_id).await
    }

    async fn find_all(
        &self,
        r1: Option<&I>,
        r2: Option<&I>,
        count: Option<usize>
    ) -> crate::db::repository::abstraction::errors::Resolve<Vec<T>> {
        SurrealDbRepository::find_all(self, r1, r2, count).await
    }

    async fn delete_one(&self, record_id: &I) -> crate::db::repository::abstraction::errors::Resolve<T> {
        SurrealDbRepository::delete_one(self, record_id).await
    }
}
