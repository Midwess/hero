#[cfg(test)]
pub mod test {
    use std::{sync::Arc, time::{Duration, Instant}};
    use tokio::sync::{OnceCell, RwLock};
    use crate::utils::pool::{allocator::{PoolAllocator, PoolBuilder, PoolResourceProvider}, cleanup::CleanupStrategy, request::PoolRequest};

    pub struct DbConnection {}

    impl DbConnection {
        pub async fn new(_: String) -> Self {
            Self {}
        }

        pub async fn query(&self) {
            tokio::time::sleep(Duration::from_millis(10)).await; // 200ms
        }
    }

    pub struct DbPoolResourceProvider {
        connection_string: String
    }

    #[async_trait::async_trait]
    impl PoolResourceProvider<DbConnection> for DbPoolResourceProvider {
        async fn new(&self) -> DbConnection {
           DbConnection::new(self.connection_string.clone()).await
        }
    }
 
    pub static DB_POOL: OnceCell<Arc<PoolAllocator<DbConnection>>> = OnceCell::const_new();

    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_pool() {
        DB_POOL.get_or_init(|| async move {
            let resource_provider = DbPoolResourceProvider {
                connection_string: String::from("mydb://root@root")
            };

            let pool = PoolBuilder::new(Box::new(resource_provider))
                .min_pool_size(1000)
                .max_pool_size(10000)
                .resource_idle_timeout(Duration::from_millis(10))
                .build().await;

            Arc::new(pool)
        }).await;

        let request = PoolRequest {
            pool: DB_POOL.get().unwrap().clone(),
            retrieving_timeout: Some(Duration::from_millis(100))
        };

        let number_of_tasks: usize = 100000;
        let success_counter: Arc<RwLock<i32>> = Arc::new(RwLock::new(0));

        let elapsed = Instant::now();
        tokio_scoped::scope(|scope| {
            for _ in 0..number_of_tasks {
                let request = request.clone();
                let success_counter = success_counter.clone();
                scope.spawn(async move {
                    let db = request.retrieve().await;
                    if db.is_none() {
                        return
                    }

                    let db = db.unwrap();
                    db.query().await;
                    drop(db);

                    let mut success_counter = success_counter.write().await;
                    *success_counter += 1;

                    // You might want to make the resource return back to the pool more early by
                    // calling drop(db)
                    // Or if you don't it still able to return to the pool at the end of it's scope
                });
            }
        });

        println!("Number of success requests are {}/{} took {}ms", success_counter.read().await, number_of_tasks, elapsed.elapsed().as_millis());
    }
}

