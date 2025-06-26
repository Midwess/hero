use bytes::Bytes;
use redb::{Database, ReadableTable, TableDefinition};

use crate::db::redb::id::RedbId;
use crate::db::redb::table::RedbTable;
use crate::db::repository::abstraction::errors::{RepositoryError, Resolve};
use crate::db::repository::abstraction::repository::{Repository, SendSync};
use crate::utils::pool::reponse::PoolResponse;

#[async_trait::async_trait]
pub trait RedbRepository<T, I>: SendSync + Repository<T, I>
where
    T: SendSync + RedbTable<I>,
    I: SendSync + RedbId
{
    async fn get_db(&self) -> PoolResponse<Database>;

    async fn create(&self, item: T) -> Resolve<T>
    where
        T: 'async_trait
    {
        let table_name = T::get_table();
        let db = self.get_db().await;

        let tx = db.begin_write()?;
        {
            let table_def: TableDefinition<&[u8], &[u8]> = TableDefinition::new(table_name);
            let mut table = tx.open_table(table_def)?;
            let id = RedbTable::id(&item);
            let key = Self::flatten_bytes_vec(id.id());
            let value = RedbTable::serialize(&item)?;

            table.insert(key.as_slice(), value.as_slice())?;
        }

        tx.commit()?;
        Ok(item)
    }

    async fn update_one(&self, item: T) -> Resolve<T>
    where
        T: 'async_trait
    {
        if RedbRepository::find_one(self, &RedbTable::id(&item)).await?.is_some() {
            // Redb will override the existing item if the key already exists
            RedbRepository::create(self, item).await
        } else {
            Err(RepositoryError::NotFound(
                format!("{:?}: {:?}", T::get_table(), RedbTable::id(&item)),
                T::get_table().to_string()
            ))
        }
    }

    async fn find_one(&self, id: &I) -> Resolve<Option<T>> {
        let table_name = T::get_table();
        let db = self.get_db().await;

        let tx = db.begin_read()?;
        let table_def: TableDefinition<&[u8], &[u8]> = TableDefinition::new(table_name);
        let table = tx.open_table(table_def)?;
        let key = Self::flatten_bytes_vec(id.id());
        let value = table.get(key.as_slice())?;
        Ok(value.map(|v| RedbTable::deserialize(v.value().to_vec()).unwrap()))
    }

    fn flatten_bytes_vec(parts: Vec<Bytes>) -> Vec<u8> {
        let total_len: usize = parts.iter().map(|b| b.len()).sum();
        let mut buf = Vec::with_capacity(total_len);
        for part in parts {
            buf.extend_from_slice(&part);
        }
        buf
    }

    async fn delete_one(&self, id: &I) -> Resolve<T> {
        let table_name = T::get_table();
        let db = self.get_db().await;
        let Some(item) = RedbRepository::find_one(self, id).await? else {
            return Err(RepositoryError::NotFound(
                format!("{:?}: {:?}", T::get_table(), id),
                T::get_table().to_string()
            ));
        };

        let tx = db.begin_write()?;
        {
            let table_def: TableDefinition<&[u8], &[u8]> = TableDefinition::new(table_name);
            let mut table = tx.open_table(table_def)?;
            let key = Self::flatten_bytes_vec(id.id());
            table.remove(key.as_slice())?;
        }
        tx.commit()?;

        Ok(item)
    }

    async fn find_all(&self, from_id: Option<&I>, to_id: Option<&I>, count: Option<usize>) -> Resolve<Vec<T>> {
        let table_name = T::get_table();
        let db = self.get_db().await;

        let tx = db.begin_read()?;
        let table_def: TableDefinition<&[u8], &[u8]> = TableDefinition::new(table_name);
        let table = tx.open_table(table_def)?;
        let from_key = from_id.map(|id| Self::flatten_bytes_vec(id.id()));
        let to_key = to_id.map(|id| Self::flatten_bytes_vec(id.id()));

        let result = match (from_key, to_key) {
            (Some(from_key), Some(to_key)) => table.range(from_key.as_slice()..=to_key.as_slice())?,
            (Some(from_key), None) => table.range(from_key.as_slice()..)?,
            (None, Some(to_key)) => table.range(..=to_key.as_slice())?,
            (None, None) => table.iter()?
        };

        let mut items = Vec::new();
        for entry in result {
            let entry = entry?;
            if items.len() >= count.unwrap_or(usize::MAX) {
                break;
            }

            items.push(RedbTable::deserialize(entry.1.value().to_vec()).unwrap());
        }

        Ok(items)
    }
}

#[cfg(not(feature = "db-remote"))]
#[cfg(not(feature = "db-local"))]
#[async_trait::async_trait]
impl<X, T, I> Repository<T, I> for X
where
    X: RedbRepository<T, I> + SendSync,
    T: SendSync + RedbTable<I> + Clone,
    I: SendSync + RedbId + Clone
{
    async fn create(&self, data: T) -> Resolve<T>
    where
        T: 'async_trait
    {
        RedbRepository::create(self, data).await
    }

    async fn update_one(&self, data: T) -> Resolve<T>
    where
        T: 'async_trait
    {
        RedbRepository::update_one(self, data).await
    }

    async fn find_one(&self, record_id: &I) -> Resolve<Option<T>> {
        RedbRepository::find_one(self, record_id).await
    }

    async fn find_all(&self, r1: Option<&I>, r2: Option<&I>, count: Option<usize>) -> Resolve<Vec<T>> {
        RedbRepository::find_all(self, r1, r2, count).await
    }

    async fn delete_one(&self, record_id: &I) -> Resolve<T> {
        RedbRepository::delete_one(self, record_id).await
    }
}
