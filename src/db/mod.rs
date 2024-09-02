pub mod builder;
pub mod trusted;

use std::ops::Deref;

use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Database;

use crate::utils::pool::allocator::PoolResource;
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

impl PoolResource<SurrealDbConnectionInfo> for SurrealDbConnection {
    async fn new(param: &SurrealDbConnectionInfo) -> Self {
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

        Self { connection: db }
    }
}
