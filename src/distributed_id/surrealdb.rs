use chrono::Utc;
use core_services::db::surrealdb::connection::SurrealDbConnection;
use core_services::utils::pool::request::PoolRequest;
use std::time::Duration;
use surreal_derive_plus::surreal_quote;
use surreal_devl::surreal_id::SurrealId;
use surreal_devl::surreal_qr::RPath;
use surrealdb::Error;
use surrealdb::sql::{Id, Thing};
use core_services::retry;
use crate::distributed_id::generator::DistributedIdGenerator;
use crate::distributed_id::generator::Worker;
use crate::distributed_id::generator::DistributedIdError;

pub static EPOCH_SINCE: u64 = 1735405200000; // 2024-12-29T00:00:00Z

pub static CONCURRENT_COLLISION_THRESHOLD: u64 = 4096;

pub static MAX_ID_IN_MS: u64 = 1024;

pub static TIMESTAMP_LENGTH: u64 = 2 ^ 41;

impl From<Error> for DistributedIdError {
    fn from(e: Error) -> Self {
        Self::WorkerError(format!("{e:?}"))
    }
}

impl SurrealId for Worker {
    fn id(&self) -> surrealdb::sql::Thing {
        Thing::from(("worker", Id::Number(self.distributed_id as i64)))
    }
}

impl Worker {
    async fn create_worker(
        distributed_id: u64,
        name: String,
        db: PoolRequest<SurrealDbConnection>
    ) -> Result<Self, DistributedIdError> {
        let db = db.retrieve().await;
        if db.is_none() {
            return Err(DistributedIdError::WorkerError("Unable to connect to database".to_owned()))
        }

        let db = db.unwrap();

        let new_worker = Worker {
            distributed_id,
            name,
            created_at: Utc::now()
        };

        let created_worker: Option<Worker> =
            db.query(surreal_quote!(r#"CREATE #record(&new_worker)"#)).await?.take(RPath::from(0))?;
        Ok(created_worker.unwrap())
    }

    async fn get_or_create_worker(
        name: String,
        db_req: PoolRequest<SurrealDbConnection>
    ) -> Result<Self, DistributedIdError> {
        use surreal_devl::surreal_qr::RPath;

        let db = db_req.retrieve().await;
        if db.is_none() {
            return Err(DistributedIdError::WorkerError("Unable to connect to database".to_owned()))
        }

        let db = db.unwrap();

        let latest_worker: Option<Worker> = db
            .query(surreal_quote!(r#"SELECT * FROM worker ORDER BY distributed_id DESC LIMIT 1"#))
            .await?
            .take(RPath::from(0))?;
        let latest_worker: Option<Worker> = latest_worker;
        if latest_worker.is_none() {
            return Self::create_worker(1, name, db_req.clone()).await
        }

        let latest_worker_node = latest_worker.unwrap();

        if latest_worker_node.distributed_id >= CONCURRENT_COLLISION_THRESHOLD * 2 {
            let latest_worker: Option<Worker> = db
                .query(surreal_quote!(
                    r#"
                    SELECT *
                    FROM worker
                    WHERE distributed_id < #(CONCURRENT_COLLISION_THRESHOLD)
                    ORDER BY distributed_id
                    DESC
                    LIMIT 1
                "#
                ))
                .await?
                .take(RPath::from(0))?;
            let latest_worker: Option<Worker> = latest_worker;
            if let Some(latest_worker) = latest_worker {
                let created_worker =
                    Self::create_worker(latest_worker.distributed_id + 1, name, db_req.clone()).await?;
                if created_worker.distributed_id == CONCURRENT_COLLISION_THRESHOLD {
                    db.query(surreal_quote!(r#"DELETE worker WHERE distributed_id > #(CONCURRENT_COLLISION_THRESHOLD)"#)).await?;
                }

                Ok(created_worker)
            } else {
                Ok(Self::create_worker(1, name, db_req.clone()).await?)
            }
        } else {
            let created_worker =
                Self::create_worker(latest_worker_node.distributed_id + 1, name, db_req.clone()).await?;
            if created_worker.distributed_id > CONCURRENT_COLLISION_THRESHOLD {
                db.query(surreal_quote!(r#"DELETE worker WHERE distributed_id <= #(CONCURRENT_COLLISION_THRESHOLD)"#)).await?;
            }

            Ok(created_worker)
        }
    }
}

impl DistributedIdGenerator {
    pub async fn init(name: String, db: PoolRequest<SurrealDbConnection>) -> Result<Self, DistributedIdError> {
        use core_services::retry;
        let delay = Duration::from_millis(500);
        let created_worker = retry!(
            retries = 10,
            delay = delay,
            |_e: &DistributedIdError| true,
            { Worker::get_or_create_worker(name.clone(), db.clone()).await }
        )?;

        Ok(Self {
            worker: created_worker,
            last_timestamp: 0,
            sequence: 0
        })
    }
}
