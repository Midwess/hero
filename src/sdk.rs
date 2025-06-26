#[cfg(feature = "db-remote")]
use std::collections::HashMap;
#[cfg(feature = "db-remote")]
use std::sync::Arc;

#[cfg(feature = "s3")]
use core_services::s3::S3Client;
#[cfg(feature = "db-remote")]
use core_services::utils::pool::allocator::{PoolAllocator, PoolBuilder};
#[cfg(feature = "db-remote")]
use core_services::utils::pool::request::PoolRequestBuilder;
#[cfg(feature = "db-remote")]
use core_services::{
    db::surrealdb::connection::{SurrealDbConnection, SurrealDbConnectionInfo, SurrealDbPoolResourceProvider},
    utils::pool::request::PoolRequest
};

#[warn(unused_imports)]
use core_services::utils::pool::allocator::PoolResourceProvider;
#[cfg(feature = "db-remote")]
use tokio::sync::Mutex;
#[cfg(feature = "s3")]
use tokio::sync::OnceCell;

#[async_trait::async_trait]
pub trait DependenciesInjection {
    #[cfg(feature = "s3")]
    fn s3_client(&self) -> S3Client;
    #[cfg(feature = "db-remote")]
    async fn system_db(&self) -> PoolRequest<SurrealDbConnection>;
    #[cfg(feature = "db-remote")]
    async fn enable_system_db(&self);
    #[cfg(feature = "db-remote")]
    async fn enable_db(&self, db_name: DbName, min_pool_size: usize, max_pool_size: usize);
    #[cfg(feature = "db-remote")]
    async fn db_allocator(&self, db_name: DbName) -> Arc<PoolAllocator<SurrealDbConnection>>;
    #[cfg(feature = "db-remote")]
    async fn db(&self, db_name: DbName) -> PoolRequest<SurrealDbConnection>;
}

type DbName = String;
pub struct DevlogSdk {
    #[cfg(feature = "db-remote")]
    db_allocators: Mutex<HashMap<DbName, Arc<PoolAllocator<SurrealDbConnection>>>>,
    #[cfg(feature = "s3")]
    s3_client: OnceCell<S3Client>
}

impl Default for DevlogSdk {
    fn default() -> Self {
        Self::new()
    }
}

impl DevlogSdk {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "db-remote")]
            db_allocators: Mutex::new(HashMap::new()),
            #[cfg(feature = "s3")]
            s3_client: OnceCell::new()
        }
    }
}

#[async_trait::async_trait]
impl DependenciesInjection for DevlogSdk {
    #[cfg(feature = "db-remote")]
    async fn enable_system_db(&self) {
        self.enable_db("system".to_string(), 0, 5).await;
    }

    #[cfg(feature = "db-remote")]
    async fn enable_db(&self, db_name: DbName, min_pool_size: usize, max_pool_size: usize) {
        use crate::config::CONFIGS;

        let db_provider: Box<dyn PoolResourceProvider<SurrealDbConnection>> = Box::new(SurrealDbPoolResourceProvider {
            info: SurrealDbConnectionInfo {
                socket_address: CONFIGS.surrealdb_ws_address.clone(),
                namespace: CONFIGS.surrealdb_namespace.clone(),
                db_name: db_name.clone(),
                db_password: CONFIGS.surrealdb_password.clone(),
                db_username: CONFIGS.surrealdb_username.clone(),
                max_pool_size,
                min_pool_size
            }
        });

        let db_allocator: Arc<PoolAllocator<SurrealDbConnection>> = PoolBuilder::new(db_provider)
            .min_pool_size(min_pool_size)
            .max_pool_size(max_pool_size)
            .build()
            .await;

        let _ = self.db_allocators.lock().await.entry(db_name).or_insert(db_allocator);
    }

    #[cfg(feature = "db-remote")]
    async fn db_allocator(&self, db_name: DbName) -> Arc<PoolAllocator<SurrealDbConnection>> {
        self.db_allocators.lock().await.get(&db_name).expect("Db allocator must be enabled").clone()
    }

    #[cfg(feature = "db-remote")]
    async fn db(&self, db_name: DbName) -> PoolRequest<SurrealDbConnection> {
        let db_allocator = self.db_allocator(db_name).await;
        PoolRequestBuilder::new().pool(db_allocator).build()
    }

    #[cfg(feature = "db-remote")]
    async fn system_db(&self) -> PoolRequest<SurrealDbConnection> {
        let db_allocator = self.db_allocator("system".to_string()).await;
        PoolRequestBuilder::new().pool(db_allocator).build()
    }

    #[cfg(feature = "s3")]
    fn s3_client(&self) -> S3Client {
        use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
        use aws_sdk_s3::Client;
        use core_services::s3::S3SleepAsync;
        use std::env;

        if let Some(s3_client) = self.s3_client.get() {
            return s3_client.clone();
        }

        let region = env::var("AWS_S3_REGION").unwrap_or("us-east-1".to_string());
        let access_key = env::var("AWS_ACCESS_KEY_ID").expect("AWS_ACCESS_KEY_ID must be defined");
        let secret_key = env::var("AWS_SECRET_ACCESS_KEY").expect("AWS_SECRET_ACCESS_KEY must be defined");
        let endpoint_url = env::var("AWS_ENDPOINT_URL").expect("AWS_ENDPOINT_URL must be defined");

        let credentials = Credentials::new(access_key, secret_key, None, None, "cloudflare");

        let mut s3_config_builder = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(region))
            .credentials_provider(credentials)
            .sleep_impl(S3SleepAsync::new());

        s3_config_builder = s3_config_builder.endpoint_url(endpoint_url).force_path_style(true);

        let client = Client::from_conf(s3_config_builder.build());

        let s3_client = S3Client {
            client: Arc::new(client)
        };

        let _ = self.s3_client.set(s3_client.clone());

        s3_client
    }
}
