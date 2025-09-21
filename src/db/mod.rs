#[cfg(feature = "db-red")]
#[cfg(not(any(feature = "db-local", feature = "db-remote")))]
pub mod redb;
pub mod repository;

#[cfg(feature = "db-idb")]
#[cfg(not(any(feature = "db-remote")))]
pub mod idb;

#[cfg(feature = "db-remote")]
pub mod surrealdb;
