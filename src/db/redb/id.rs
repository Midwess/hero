use crate::db::repository::abstraction::id::DbId;
use crate::db::repository::abstraction::repository::SendSync;
use bytes::Bytes;

pub trait RedbId: DbId + SendSync + std::fmt::Debug {
    fn id(&self) -> Vec<Bytes>;
}
