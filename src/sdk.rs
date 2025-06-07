#[cfg(feature = "s3")]
use core_services::s3::S3Client;
#[cfg(feature = "db-remote")]
use core_services::utils::pool::allocator::{PoolAllocator, PoolBuilder};
#[cfg(feature = "db-remote")]
use core_services::utils::pool::request::PoolRequestBuilder;
#[cfg(feature = "db-remote")]
use core_services::{
    db::remote_surrealdb::{SurrealDbConnection, SurrealDbConnectionInfo, SurrealDbPoolResourceProvider},
    utils::pool::request::PoolRequest
};

#[warn(unused_imports)]
use core_services::utils::pool::allocator::PoolResourceProvider;

#[async_trait::async_trait]
pub trait DependenciesInjection {
    #[cfg(feature = "s3")]
    fn s3_client(&self, bucket_name: String) -> S3Client;
    #[cfg(feature = "s3")]
    fn cloud_flare_s3_client(&self, bucket_name: String) -> S3Client;
    #[cfg(feature = "db-remote")]
    fn system_db(&self) -> PoolRequest<SurrealDbConnection>;
    #[cfg(feature = "db-remote")]
    fn app_db(&self) -> PoolRequest<SurrealDbConnection>;
    #[cfg(feature = "db-remote")]
    async fn enable_system_db(&self);
    #[cfg(feature = "db-remote")]
    async fn enable_app_db(&self, app_name: String);
}

pub struct DevlogSdk {
    #[cfg(feature = "db-remote")]
    system_db_allocator: OnceCell<Arc<PoolAllocator<SurrealDbConnection>>>,
    #[cfg(feature = "db-remote")]
    app_db_allocator: OnceCell<Arc<PoolAllocator<SurrealDbConnection>>>
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
            system_db_allocator: OnceCell::const_new(),
            #[cfg(feature = "db-remote")]
            app_db_allocator: OnceCell::const_new()
        }
    }
}

#[async_trait::async_trait]
impl DependenciesInjection for DevlogSdk {
    #[cfg(feature = "db-remote")]
    async fn enable_system_db(&self) {
        use crate::config::CONFIGS;

        let min_pool_size = 1;
        let max_pool_size = 5;

        let db_provider: Box<dyn PoolResourceProvider<SurrealDbConnection>> = Box::new(SurrealDbPoolResourceProvider {
            info: SurrealDbConnectionInfo {
                socket_address: CONFIGS.surrealdb_ws_address.clone(),
                namespace: CONFIGS.environment.clone(),
                db_name: "system".to_string(),
                db_password: CONFIGS.surrealdb_password.clone(),
                db_username: "devlog".to_string(),
                max_pool_size,
                min_pool_size
            }
        });

        let db_allocator: Arc<PoolAllocator<SurrealDbConnection>> = PoolBuilder::new(db_provider)
            .min_pool_size(min_pool_size)
            .max_pool_size(max_pool_size)
            .build()
            .await;

        let _ = self.system_db_allocator.set(db_allocator);
    }

    #[cfg(feature = "db-remote")]
    async fn enable_app_db(&self, app_name: String) {
        use crate::config::CONFIGS;

        let min_pool_size = 1;
        let max_pool_size = 5;

        let db_provider: Box<dyn PoolResourceProvider<SurrealDbConnection>> = Box::new(SurrealDbPoolResourceProvider {
            info: SurrealDbConnectionInfo {
                socket_address: CONFIGS.surrealdb_ws_address.clone(),
                namespace: CONFIGS.environment.clone(),
                db_name: app_name,
                db_password: CONFIGS.surrealdb_password.clone(),
                db_username: "devlog".to_string(),
                max_pool_size,
                min_pool_size
            }
        });

        let db_allocator: Arc<PoolAllocator<SurrealDbConnection>> = PoolBuilder::new(db_provider)
            .min_pool_size(min_pool_size)
            .max_pool_size(max_pool_size)
            .build()
            .await;

        let _ = self.app_db_allocator.set(db_allocator);
    }

    #[cfg(feature = "db-remote")]
    fn system_db(&self) -> PoolRequest<SurrealDbConnection> {
        PoolRequestBuilder::new()
            .pool(self.system_db_allocator.get().expect("System db allocator must be enabled").clone())
            .build()
    }

    #[cfg(feature = "db-remote")]
    fn app_db(&self) -> PoolRequest<SurrealDbConnection> {
        PoolRequestBuilder::new()
            .pool(self.app_db_allocator.get().expect("App db allocator must be enabled").clone())
            .build()
    }

    #[cfg(feature = "s3")]
    fn s3_client(&self, bucket_name: String) -> S3Client {
        use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
        use aws_sdk_s3::Client;
        use core_services::s3::S3SleepAsync;
        use std::env;

        let region = env::var("AWS_S3_REGION").unwrap_or("us-east-1".to_string());
        let access_key = env::var("AWS_ACCESS_KEY_ID").expect("AWS_ACCESS_KEY_ID must be defined");
        let secret_key = env::var("AWS_SECRET_ACCESS_KEY").expect("AWS_SECRET_ACCESS_KEY must be defined");
        let endpoint_url = env::var("AWS_ENDPOINT_URL").expect("AWS_ENDPOINT_URL must be defined");

        let credentials = Credentials::new(access_key, secret_key, None, None, "devlog");

        let mut s3_config_builder = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(region))
            .credentials_provider(credentials)
            .sleep_impl(S3SleepAsync::new());

        s3_config_builder = s3_config_builder.endpoint_url(endpoint_url).force_path_style(true);

        let client = Client::from_conf(s3_config_builder.build());

        S3Client { client, bucket_name }
    }

    #[cfg(feature = "s3")]
    fn cloud_flare_s3_client(&self, bucket_name: String) -> S3Client {
        use std::env;

        use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
        use aws_sdk_s3::Client;
        use core_services::s3::S3SleepAsync;

        let region = env::var("CLOUD_FLARE_R2_REGION").unwrap_or("us-east-1".to_string());
        let access_key =
            env::var("CLOUD_FLARE_R2_ACCESS_KEY_ID").expect("CLOUD_FLARE_R2_ACCESS_KEY_ID must be defined");
        let secret_key =
            env::var("CLOUD_FLARE_R2_SECRET_ACCESS_KEY").expect("CLOUD_FLARE_R2_SECRET_ACCESS_KEY must be defined");
        let endpoint_url =
            env::var("CLOUD_FLARE_R2_ENDPOINT_URL").expect("CLOUD_FLARE_R2_ENDPOINT_URL must be defined");

        let credentials = Credentials::new(access_key, secret_key, None, None, "devlog-cloudflare");

        let mut s3_config_builder = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(region))
            .credentials_provider(credentials)
            .sleep_impl(S3SleepAsync::new());

        s3_config_builder = s3_config_builder.endpoint_url(endpoint_url).force_path_style(true);

        let client = Client::from_conf(s3_config_builder.build());

        S3Client { client, bucket_name }
    }
}
