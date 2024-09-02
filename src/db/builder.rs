use surreal_derive_plus::surreal_quote;
use surreal_devl::serialize::SurrealSerialize;
use surrealdb::opt::RecordId;
use surrealdb::sql::Value;

use crate::services::base::Resolve;
use crate::utils::pool::reponse::PoolResponse;

use super::trusted::TrustedOne;
use super::{SurrealDbConnection, SurrealDbConnectionInfo};

pub trait SendSync = Sized + Sync + Send;
pub trait SurrealDerive = SendSync + SurrealSerialize + Into<RecordId,> + From<Value,>;

#[async_trait::async_trait]
pub trait Repository<T, I,>
where
    T: SendSync,
    I: SendSync,
{
    async fn create(&self, data: &T,) -> Resolve<T,>;

    async fn get(&self, id: &I,) -> Resolve<Option<T,>,>;

    async fn delete(&self, id: &I,) -> Resolve<Option<T,>,>;
}

#[async_trait::async_trait]
pub trait SurrealDbRepository<T, I,>: Repository<T, I,>
where
    T: SurrealDerive,
    I: SurrealDerive,
{
    async fn get_db(&self,) -> PoolResponse<SurrealDbConnection, SurrealDbConnectionInfo,>;

    async fn create(&self, data: &T,) -> Resolve<T,> {
        let db = self.get_db().await;
        let result: TrustedOne = db.query(surreal_quote!("CREATE #record(data)"),).await?.take(0,)?;
        let value: Value = result.into();
        Ok(value.into(),)
    }

    async fn get(&self, record_id: &I,) -> Resolve<Option<T,>,> {
        let db = self.get_db().await;
        let result: TrustedOne = db.query(surreal_quote!("SELECT * FROM #id(record_id)"),).await?.take(0,)?;
        Ok(result.into(),)
    }

    async fn delete(&self, record_id: &I,) -> Resolve<Option<T,>,> {
        let db = self.get_db().await;
        let result: TrustedOne =
            db.query(surreal_quote!("UPDATE #id(record_id) SET deleted_at=time::now()"),).await?.take(0,)?;
        Ok(result.into(),)
    }
}

#[async_trait::async_trait]
impl<X, T, I,> Repository<T, I,> for X
where
    X: SurrealDbRepository<T, I,> + SendSync,
    T: SurrealDerive,
    I: SurrealDerive,
{
    async fn create(&self, data: &T,) -> Resolve<T,> {
        SurrealDbRepository::create(self, data,).await
    }

    async fn get(&self, record_id: &I,) -> Resolve<Option<T,>,> {
        SurrealDbRepository::get(self, record_id,).await
    }

    async fn delete(&self, record_id: &I,) -> Resolve<Option<T,>,> {
        SurrealDbRepository::delete(self, record_id,).await
    }
}
