use chrono::{DateTime, Utc};
use core_services::db::remote_surrealdb::SurrealDbConnection;
use core_services::utils::pool::request::PoolRequest;
use generator::{DistributedIdGenerator, EPOCH_SINCE};
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};

pub mod db;
pub mod generator;

pub static DISTRIBUTED_ID_GENERATOR: OnceCell<Arc<Mutex<DistributedIdGenerator>>> = OnceCell::const_new();

pub async fn gen_id() -> u64 {
    let generator = DISTRIBUTED_ID_GENERATOR.get().unwrap();
    generator.lock().await.next_id().await.unwrap()
}

pub fn id_to_unix_timestamp(id: u64) -> u64 {
    let timestamp = id >> 23;
    let epoch_since_ms = EPOCH_SINCE;
    epoch_since_ms + timestamp
}

pub async fn init_id_generator(app_name: String, db: PoolRequest<SurrealDbConnection>) {
    let distributed_id_generator = DistributedIdGenerator::init(app_name.clone(), db).await.unwrap();
    let _ = DISTRIBUTED_ID_GENERATOR.set(Arc::new(Mutex::new(distributed_id_generator)));
}

pub fn init_scoped_id_generator(app_name: String) {
    let distributed_id_generator = DistributedIdGenerator::init_scoped(app_name.clone());
    let _ = DISTRIBUTED_ID_GENERATOR.set(Arc::new(Mutex::new(distributed_id_generator)));
}

pub fn id_to_datetime(id: u64) -> DateTime<Utc> {
    let timestamp = id >> 23;
    let epoch_since_ms = EPOCH_SINCE;
    DateTime::from_timestamp_millis(epoch_since_ms as i64 + timestamp as i64).unwrap()
}
