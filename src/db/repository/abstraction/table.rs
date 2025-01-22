use surreal_devl::surreal_id::SurrealId;
use surrealdb::sql::Thing;

use super::id::DbId;

pub trait Table<T>
where
    T: DbId
{
    fn get_table() -> &'static str;
    fn id(&self) -> T;
}