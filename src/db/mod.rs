pub mod repository;
pub mod result;
use std::ops::Deref;

use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Database;

use crate::utils::pool::allocator::PoolResourceProvider;
use crate::Db;

#[derive(Debug, Clone)]
pub struct SurrealDbConnectionInfo {
    pub socket_address: String,
    /// https://surrealdb.com/docs/surrealql/statements/define/namespace
    pub namespace: String,
    pub db_name: String,
    pub db_password: String,
    pub db_username: String
}

pub struct SurrealDbConnection {
    connection: Db
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
        db.connect::<Ws>(param.socket_address.clone()).await.expect("Failed while connecting to surreal db");
        db.signin(Database {
            database: param.db_name.as_str(),
            namespace: param.namespace.as_str(),
            username: param.db_username.as_str(),
            password: param.db_password.as_str()
        })
        .await
        .unwrap();

        log::info!(target: "db", "Connected to surreal db using {:?}", param);
        SurrealDbConnection { connection: db }
    }
}
