use chrono::{DateTime, Utc};
use core_services::db::trusted::TrustedOne;
use core_services::db::SurrealDbConnection;
use core_services::retry;
use core_services::utils::pool::request::PoolRequest;
use once_cell::sync::Lazy;
use schema::misc::datetime::Datetime as DbDateTime;
use std::time::Duration;
use surreal_derive_plus::{surreal_quote, SurrealDerive};
use surrealdb::opt::RecordId;
use surrealdb::sql::Id;
use surrealdb::Error;
use thiserror::Error;

pub static EPOCH_SINCE: Lazy<DateTime<Utc>> =
    Lazy::new(|| "2024-12-29T00:00:00Z".parse::<DateTime<Utc>>().expect("EPOCH_SINCE is not valid"));

pub static CONCURRENT_COLLISION_THRESHOLD: u64 = (2 ^ 13) / 2;

pub static MAX_ID_IN_MS: u64 = 1024;

pub static TIMESTAMP_LENGTH: u64 = 2 ^ 41;

#[derive(Error, Debug)]
pub enum DistributedIdError {
    #[error("Unnable to connect to database")]
    UnnableToConnectToDatabase,
    #[error("Db query error {:?}", .0)]
    DbError(#[from] Error),
    #[error("Worker out of bounds of database {} worker name {}", .0, .1)]
    WorkerOutOfBound(String, String)
}

#[derive(Debug, SurrealDerive, Clone)]
pub struct Worker {
    distributed_id: u64,
    name: String,
    created_at: DbDateTime
}

impl Worker {
    async fn create_worker(
        distributed_id: u64,
        name: String,
        db: PoolRequest<SurrealDbConnection>
    ) -> Result<Self, DistributedIdError> {
        let db = db.retrieve().await;
        if db.is_none() {
            return Err(DistributedIdError::UnnableToConnectToDatabase)
        }

        let db = db.unwrap();

        let new_worker = Worker {
            distributed_id,
            name,
            created_at: DbDateTime::now()
        };

        let created_worker: TrustedOne = db.query(surreal_quote!(r#"CREATE #record(&new_worker)"#)).await?.take(0)?;
        let created_worker: Option<Worker> = created_worker.into();
        return Ok(created_worker.unwrap())
    }

    async fn get_or_create_worker(
        name: String,
        db_req: PoolRequest<SurrealDbConnection>
    ) -> Result<Self, DistributedIdError> {
        let db = db_req.retrieve().await;
        if db.is_none() {
            return Err(DistributedIdError::UnnableToConnectToDatabase)
        }

        let db = db.unwrap();

        let latest_worker: TrustedOne = db
            .query(surreal_quote!(r#"SELECT * FROM worker ORDER BY distributed_id DESC LIMIT 1"#))
            .await?
            .take(0)?;
        let latest_worker: Option<Worker> = latest_worker.into();
        if latest_worker.is_none() {
            return Self::create_worker(1, name, db_req.clone()).await
        }

        let latest_worker_node = latest_worker.unwrap();

        if latest_worker_node.distributed_id >= CONCURRENT_COLLISION_THRESHOLD * 2 {
            let latest_worker: TrustedOne = db.query(surreal_quote!(r#"SELECT * FROM distributed_id < r#(CONCURRENT_COLLISION_THRESHOLD - 1) ORDER BY distributed_id DESC LIMIT 1"#)).await?.take(0)?;
            let latest_worker: Option<Worker> = latest_worker.into();
            if let Some(latest_worker) = latest_worker {
                let created_worker =
                    Self::create_worker(latest_worker.distributed_id + 1, name, db_req.clone()).await?;
                if created_worker.distributed_id == CONCURRENT_COLLISION_THRESHOLD {
                    db.query(r#"DELETE worker WHERE distributed_id > #(CONCURRENT_COLLISION_THRESHOLD)"#).await?;
                }

                return Ok(created_worker)
            } else {
                return Self::create_worker(1, name, db_req.clone()).await;
            }
        } else {
            let created_worker =
                Self::create_worker(latest_worker_node.distributed_id + 1, name, db_req.clone()).await?;
            if created_worker.distributed_id > CONCURRENT_COLLISION_THRESHOLD {
                db.query(r#"DELETE worker WHERE distributed_id <= #(CONCURRENT_COLLISION_THRESHOLD)"#).await?;
            }

            Ok(created_worker)
        }
    }
}

impl From<Worker> for RecordId {
    fn from(value: Worker) -> Self {
        Self::from(("worker", Id::Number(value.distributed_id as i64)))
    }
}

pub struct DistributedIdGenerator {
    pub worker: Worker,
    pub last_timestamp: i64,
    // Sequence counter for IDs generated in that same ms
    pub sequence: u64
}

impl DistributedIdGenerator {
    pub async fn init(name: String, db: PoolRequest<SurrealDbConnection>) -> Result<Self, DistributedIdError> {
        let created_worker = retry!(
            retries = 3,
            delay = std::time::Duration::new(0, 100000000), // 100 ms
            |_e: &DistributedIdError| { return true },
            { Worker::get_or_create_worker(name.clone(), db.clone()).await }
        )?;

        Ok(Self {
            worker: created_worker,
            last_timestamp: 0,
            sequence: 0
        })
    }

    /// Generates a 64-bit ID with the layout:
    /// [41 bits: timestamp in seconds since EPOCH_SINCE]
    /// [13 bits: worker ID]
    /// [10 bits: sequence for that second]
    async fn next_id(&mut self) -> Result<i64, DistributedIdError> {
        let now_utc = Utc::now();
        let epoch_since_seconds = EPOCH_SINCE.timestamp();
        let current_seconds = now_utc.timestamp();

        let mut timestamp = current_seconds - epoch_since_seconds;
        if timestamp < 0 {
            timestamp = 0;
        }

        if timestamp == self.last_timestamp {
            self.sequence += 1;

            if self.sequence >= MAX_ID_IN_MS {
                // Wait until we move to the next millisec
                while timestamp == self.last_timestamp {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                    let new_now_utc = Utc::now();
                    let new_seconds = new_now_utc.timestamp() - epoch_since_seconds;
                    if new_seconds > timestamp {
                        timestamp = new_seconds;
                        break;
                    }
                }

                self.sequence = 0;
                self.last_timestamp = timestamp;
            }
        } else {
            self.sequence = 0;
            self.last_timestamp = timestamp;
        }

        // Construct the 64-bit ID
        // 41 bits of timestamp | 13 bits of worker | 10 bits of sequence
        let timestamp_41 = (timestamp as i64) & ((1 << 41) - 1);
        let worker_13 = (self.worker.distributed_id as i64) & ((1 << 13) - 1);
        let seq_10 = (self.sequence as i64) & ((1 << 10) - 1);

        let id = (timestamp_41 << 23) | (worker_13 << 10) | seq_10;

        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use schema::misc::datetime::Datetime as DbDateTime;

    #[tokio::test]
    async fn test_next_id_increments() -> Result<(), Box<dyn std::error::Error>> {
        let worker = Worker {
            distributed_id: 42,
            name: "test_worker".to_string(),
            created_at: DbDateTime::now()
        };

        let mut generator = DistributedIdGenerator {
            worker,
            last_timestamp: 0,
            sequence: 0
        };

        let first_id = generator.next_id().await?;
        let second_id = generator.next_id().await?;

        assert!(second_id > first_id, "second_id should be greater than first_id");

        let mut prev_id = second_id;
        for _ in 0..10 {
            let next = generator.next_id().await?;
            assert!(next > prev_id, "IDs should be strictly increasing");
            prev_id = next;
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_sequence_rollover() -> Result<(), Box<dyn std::error::Error>> {
        let worker = Worker {
            distributed_id: 1,
            name: "rollover_test".to_string(),
            created_at: DbDateTime::now()
        };

        let mut generator = DistributedIdGenerator {
            worker,
            last_timestamp: 1000,       // pretend we're at "second 1000"
            sequence: MAX_ID_IN_MS - 1  // about to overflow
        };

        let first_id = generator.next_id().await?;
        assert_eq!(generator.sequence, 0, "sequence should have reset to 0");
        assert!(generator.last_timestamp >= 1000, "timestamp should move forward if we waited");

        let second_id = generator.next_id().await?;
        assert!(second_id > first_id, "IDs should keep increasing even after rollover");

        Ok(())
    }
}
