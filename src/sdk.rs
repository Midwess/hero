#[cfg(feature = "db-remote")]
use core_services::db::remote_surrealdb::{SurrealDbConnection, SurrealDbPoolResourceProvider};
#[cfg(feature = "s3")]
use core_services::s3::{S3Client, S3ClientResourceProvider};

#[warn(unused_imports)]
use core_services::utils::pool::allocator::PoolResourceProvider;

#[async_trait::async_trait]
pub trait DependenciesInjection {
    #[cfg(feature = "s3")]
    fn s3_client(&self) -> S3Client;
    #[cfg(feature = "db-remote")]
    fn db(&self) -> PoolRequest<SurrealDbConnection>;
    #[cfg(feature = "s3")]
    async fn enable_s3(&self, min_size: usize, max_size: usize);
    #[cfg(feature = "db-remote")]
    async fn enable_db(&self, min_size: usize, max_size: usize);
}

pub struct DevlogSdk {
    #[cfg(feature = "s3")]
    s3_allocator: OnceCell<Arc<PoolAllocator<aws_sdk_s3::client::Client>>>,

    #[cfg(feature = "db-remote")]
    devlog_db_allocator: OnceCell<Arc<PoolAllocator<SurrealDbConnection>>>
}

impl Default for DevlogSdk {
    fn default() -> Self {
        Self::new()
    }
}

impl DevlogSdk {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "s3")]
            s3_allocator: OnceCell::const_new(),
            #[cfg(feature = "db-remote")]
            devlog_db_allocator: OnceCell::const_new()
        }
    }
}

#[async_trait::async_trait]
impl DependenciesInjection for DevlogSdk {
    #[cfg(feature = "s3")]
    async fn enable_s3(&self, min_size: usize, max_size: usize) {
        let s3_provider: Box<dyn PoolResourceProvider<aws_sdk_s3::Client>> = Box::new(S3ClientResourceProvider {});
        let s3_allocator: Arc<PoolAllocator<aws_sdk_s3::Client>> =
            PoolBuilder::new(s3_provider).min_pool_size(min_size).max_pool_size(max_size).build().await;

        let _ = self.s3_allocator.set(s3_allocator);
    }

    #[cfg(feature = "db-remote")]
    async fn enable_db(&self, min_size: usize, max_size: usize) {
        let db_provider: Box<dyn PoolResourceProvider<SurrealDbConnection>> = Box::new(SurrealDbPoolResourceProvider {
            info: CONFIGS.surrealdb.clone()
        });

        let db_allocator: Arc<PoolAllocator<SurrealDbConnection>> =
            PoolBuilder::new(db_provider).min_pool_size(min_size).max_pool_size(max_size).build().await;

        let _ = self.devlog_db_allocator.set(db_allocator);
    }

    #[cfg(feature = "db-remote")]
    fn db(&self) -> PoolRequest<SurrealDbConnection> {
        PoolRequestBuilder::new()
            .pool(self.devlog_db_allocator.get().expect("Devlog db allocator must be enabled").clone())
            .build()
    }

    #[cfg(feature = "s3")]
    fn s3_client(&self) -> S3Client {
        S3Client {
            client: PoolRequestBuilder::new()
                .pool(self.s3_allocator.get().expect("S3 allocator must be enabled").clone())
                .build()
        }
    }
}
