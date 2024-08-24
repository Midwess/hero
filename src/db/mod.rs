use core_services::Db;
use log::info;
use once_cell::sync::Lazy;
use surrealdb::{engine::remote::ws::Ws, opt::auth::Root, Surreal};

use crate::config::CONFIGS;

pub static DB: Lazy<Db> = Lazy::new(Surreal::init);

pub static NS: &'static str = "devlog-sdk";

pub async fn setup_devlog_db() -> Result<(), Box<dyn std::error::Error>> {
    let ns = format!("{NS} devlog-sdk-hub-db");
    DB.connect::<Ws>(CONFIGS.surrealdb.socket_address.clone()).await.expect("Failed while connecting to surreal db");
    DB.use_ns(CONFIGS.surrealdb.namespace.clone()).use_db(CONFIGS.surrealdb.db_name.clone()).await.unwrap();
    DB.signin(Root {
        username: CONFIGS.surrealdb.db_username.clone().as_str(),
        password: CONFIGS.surrealdb.db_password.clone().as_str()
    }).await.unwrap();

    let db_version = DB.version().await.expect("Failed to get the surreal db version");

    info!(target: &ns, "Connected to SurrealDb version: {} {}", db_version, CONFIGS.surrealdb.socket_address);

    Ok(())
}

