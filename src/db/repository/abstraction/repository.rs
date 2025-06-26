use super::errors::Resolve;
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
