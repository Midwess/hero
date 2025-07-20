#[cfg(feature = "db-red")]
#[cfg(not(any(feature = "db-local", feature = "db-remote")))]
pub mod redb;
pub mod repository;
#[cfg(any(feature = "db-local", feature = "db-remote"))]
#[cfg(not(feature = "db-red"))]
pub mod surrealdb;

#[cfg(feature = "db-idb")]
#[cfg(not(any(feature = "db-local", feature = "db-remote")))]
pub mod idb;
