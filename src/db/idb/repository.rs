use idb::{Database, KeyPath, KeyRange, ObjectStoreParams, Query, TransactionMode};
use crate::db::repository::abstraction::errors::{RepositoryError, Resolve};
use crate::db::repository::abstraction::repository::{Repository, SendSync};
use crate::db::idb::table::IdbTable;
use crate::db::idb::id::IdbId;
use crate::utils::never_send::NeverSend;
use crate::utils::pool::reponse::PoolResponse;

#[async_trait::async_trait(?Send)]
pub trait IdbRepository<T, I>: Send + Sync + Repository<T, I>
where
    T: Send + Sync + IdbTable<I>,
    I: Send + Sync + IdbId
{
    async fn get_db(&self) -> PoolResponse<NeverSend<Database>>;

    async fn create(&self, item: T) -> Resolve<T>
    where
        T: 'async_trait
    {
        let table_name = T::get_table();

        let db = self.get_db().await;

        let transaction = db.transaction(&[table_name], TransactionMode::ReadWrite)?;
        let store = transaction.object_store(table_name)?;
        let id = IdbTable::id(&item);
        let key = IdbId::serialize(&id)?;
        let value = IdbTable::serialize(&item)?;

        let inserted_key = store.add(&value, Some(&key))?.await?;
        let Some(inserted_value) = store.get(inserted_key)?.await? else {
            return Err(RepositoryError::NotFound(table_name.to_owned(), format!("{:?}", id)))
        };

        let inserted_item: T = IdbTable::deserialize(inserted_value)?;

        Ok(inserted_item)
    }

    async fn find_one(&self, id: &I) -> Resolve<Option<T>> {
        let mut result = IdbRepository::find_all(self, Some(id), None, Some(1)).await?;
        if result.is_empty() {
            return Ok(None);
        }

        Ok(Some(result.remove(0)))
    }

    async fn find_all(&self, from: Option<&I>, to: Option<&I>, limit: Option<usize>) -> Resolve<Vec<T>> {
        let table_name = T::get_table();
        let db = self.get_db().await;

        let from_id = match from {
            Some(id) => {
                let id = IdbId::serialize(id)?;
                Some(id)
            },
            None => None
        };

        let to_id = match to {
            Some(to_id) => {
                let to_id = IdbId::serialize(to_id)?;
                Some(to_id)
            },
            None => None
        };

        log::info!("from_id: {:?}, to_id: {:?}", from_id, to_id);
        let range_query = match (from_id, to_id) {
            (Some(from_id), Some(to_id)) => Some(Query::KeyRange(KeyRange::bound(&from_id, &to_id, Some(false), Some(false))?)),
            (Some(from_id), None) => Some(Query::KeyRange(KeyRange::lower_bound(&from_id, Some(false))?)),
            (None, Some(to_id)) => Some(Query::KeyRange(KeyRange::upper_bound(&to_id, Some(false))?)),
            (None, None) => None
        };

        let transaction = db
            .transaction(&[table_name], TransactionMode::ReadOnly)?;
        let store = transaction.object_store(table_name)?;
        let result = store.get_all(range_query, limit.map(|it| it as u32))?.await?;

        Ok(result.into_iter().filter_map(|it| {
            match IdbTable::deserialize(it) {
                Err(e) => {
                    log::error!("Error deserializing record: {:?}", e);
                    None
                },
                Ok(record) => Some(record)
            }
        }).collect())
    }

    async fn delete_one(&self, id: &I) -> Resolve<T> {
        let table_name = T::get_table();

        let record = IdbRepository::find_one(self, id).await?;
        if record.is_none() {
            return Err(RepositoryError::NotFound(table_name.to_string(), format!("{:?}", id)))
        }

        let db = self.get_db().await;
        let transaction = db.transaction(&[table_name], TransactionMode::ReadWrite)?;
        let store = transaction.object_store(table_name)?;
        let key = IdbId::serialize(id)?;
        let query = Query::Key(key);
        store.delete(query)?.await?;
        Ok(record.unwrap())
    }

    async fn update_one(&self, item: T) -> Resolve<T>
    where
        T: 'async_trait
    {
        let id = IdbTable::id(&item);
        let table_name = T::get_table();

        let record = IdbRepository::find_one(self, &id).await?;
        if record.is_none() {
            return Err(RepositoryError::NotFound(table_name.to_owned(), format!("{:?}", IdbTable::id(&item))))
        }

        let db = self.get_db().await;
        let table_name = T::get_table();
        let transaction = db.transaction(&[table_name], TransactionMode::ReadWrite)?;
        let store = transaction.object_store(table_name)?;
        let id = IdbTable::id(&item);
        let key = IdbId::serialize(&id)?;
        let value = IdbTable::serialize(&item)?;
        let updated = store.put(&value, Some(&key))?.await?;
        Ok(IdbTable::deserialize(updated)?)
    }
}

#[cfg_attr(not(target_family = "wasm"), async_trait::async_trait)]
#[cfg_attr(target_family = "wasm", async_trait::async_trait(?Send))]
impl<X, T, I> Repository<T, I> for X
where
    X: IdbRepository<T, I> + Send + Sync,
    T: Send + Sync + IdbTable<I> + Clone,
    I: Send + Sync + IdbId + Clone
{
    async fn create(&self, data: T) -> Resolve<T>
    where
        T: 'async_trait
    {
        IdbRepository::create(self, data).await
    }

    async fn update_one(&self, data: T) -> Resolve<T>
    where
        T: 'async_trait
    {
        IdbRepository::update_one(self, data).await
    }

    async fn find_one(&self, record_id: &I) -> Resolve<Option<T>> {
        IdbRepository::find_one(self, record_id).await
    }

    async fn find_all(&self, r1: Option<&I>, r2: Option<&I>, count: Option<usize>) -> Resolve<Vec<T>> {
        IdbRepository::find_all(self, r1, r2, count).await
    }

    async fn delete_one(&self, record_id: &I) -> Resolve<T> {
        IdbRepository::delete_one(self, record_id).await
    }
}
