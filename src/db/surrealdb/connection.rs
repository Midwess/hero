use std::ops::Deref;

use surrealdb::opt::auth::Namespace;

use crate::utils::pool::allocator::PoolResourceProvider;
use crate::Db;

#[derive(Debug, Clone)]
pub struct SurrealDbConnectionInfo {
    pub socket_address: String,
    pub namespace: String,
    pub db_name: String,
    pub db_password: String,
    pub db_username: String,
    pub max_pool_size: usize,
    pub min_pool_size: usize
}

pub struct SurrealDbConnection {
    pub connection: Db
}

impl Deref for SurrealDbConnection {
    type Target = Db;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

pub struct SurrealDbPoolResourceProvider {
    pub info: SurrealDbConnectionInfo
}

#[async_trait::async_trait]
impl PoolResourceProvider<SurrealDbConnection> for SurrealDbPoolResourceProvider {
    async fn new(&self) -> SurrealDbConnection {
        let param = &self.info;
        let db = Db::init();
        db.connect(param.socket_address.clone()).await.expect("Failed while connecting to surreal db");
        db.signin(Namespace {
            namespace: param.namespace.as_str(),
            username: param.db_username.as_str(),
            password: param.db_password.as_str()
        })
        .await
        .unwrap();

        db.use_db(param.db_name.as_str()).await.unwrap();

        log::info!(target: "db", "Connected to surreal db using {:?}", param);
        SurrealDbConnection { connection: db }
    }
}
