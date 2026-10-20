//! Snowflake-style identifiers backed by a process-global generator.
//!
//! Initialize exactly once before generating IDs. [`init_scoped_id_generator`]
//! assigns the reserved process-local worker ID; use
//! [`init_scoped_id_generator_with_id`] when deployment already guarantees a
//! unique worker ID. With `distributed-id-etcd`, `init_id_generator` reserves
//! a worker ID under a renewable etcd lease.
//!
//! IDs sort by generation time and contain a millisecond timestamp, a 13-bit
//! worker ID, and a 10-bit per-millisecond sequence. They are infrastructure
//! identifiers, not secrets: callers can recover their creation time with
//! [`id_to_datetime`].
//!
//! # Example
//!
//! ```
//! # futures::executor::block_on(async {
//! use hero::distributed_id::{id_to_datetime, init_scoped_id_generator, try_gen_id};
//!
//! init_scoped_id_generator("thumbnail-worker").unwrap();
//! let id = try_gen_id().await.unwrap();
//! assert!(id_to_datetime(id).is_ok());
//! # });
//! ```

use chrono::{DateTime, Utc};
use generator::{DistributedIdError, DistributedIdGenerator};
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};

#[cfg(all(feature = "distributed-id-etcd", not(target_family = "wasm")))]
/// etcd-backed worker allocation and lease management.
pub mod etcd;
/// The low-level Snowflake generator and encoded layout.
pub mod generator;
/// Process-local synthetic phone-number helpers.
pub mod scoped;

#[cfg(all(feature = "distributed-id-etcd", not(target_family = "wasm")))]
pub use etcd::EtcdWorkerOptions;
pub use generator::EPOCH_SINCE;
pub use scoped::{
    format_phone_number, generate_phone_number, RegionCodeError, REGION_CODE_MAX, REGION_CODE_MIN, SUBSCRIBER_MOD,
};

static DISTRIBUTED_ID_GENERATOR: OnceCell<Arc<Mutex<DistributedIdGenerator>>> = OnceCell::const_new();
#[cfg(all(feature = "distributed-id-etcd", not(target_family = "wasm")))]
static WORKER_LEASE_GUARD: OnceCell<etcd::WorkerLeaseGuard> = OnceCell::const_new();

/// Generates an ID from the initialized process-global generator.
///
/// The call is serialized with other generation calls. When 1,024 IDs are
/// requested within one millisecond, it waits for the clock to advance.
pub async fn try_gen_id() -> Result<u64, DistributedIdError> {
    let generator = DISTRIBUTED_ID_GENERATOR.get().ok_or(DistributedIdError::NotInitialized)?;
    generator.lock().await.next_id().await
}

#[deprecated(note = "use try_gen_id")]
/// Generates an ID, panicking when the process-global generator is uninitialized.
pub async fn gen_id() -> u64 {
    try_gen_id().await.expect("distributed ID generator is not initialized")
}

#[cfg(not(target_family = "wasm"))]
/// Blocks the current native thread while generating an ID.
///
/// Prefer [`try_gen_id`] inside async code.
pub fn try_gen_id_sync() -> Result<u64, DistributedIdError> {
    n0_future::future::block_on(try_gen_id())
}

#[cfg(not(target_family = "wasm"))]
#[deprecated(note = "use try_gen_id_sync")]
/// Generates an ID synchronously, panicking when initialization is missing.
pub fn gen_id_sync() -> u64 {
    try_gen_id_sync().expect("distributed ID generator is not initialized")
}

/// Extracts the encoded Unix timestamp in milliseconds.
pub fn id_to_unix_timestamp(id: u64) -> u64 {
    EPOCH_SINCE + (id >> 23)
}

/// Converts an encoded ID into its UTC generation time.
pub fn id_to_datetime(id: u64) -> Result<DateTime<Utc>, DistributedIdError> {
    let timestamp = id_to_unix_timestamp(id);
    DateTime::from_timestamp_millis(timestamp as i64).ok_or(DistributedIdError::InvalidTimestamp(id))
}

/// Initializes the process-global generator for a single-process deployment.
///
/// This mode uses the highest non-reserved worker slot. It must not be used by
/// multiple processes that can generate IDs for the same data set.
pub fn init_scoped_id_generator(app_name: impl Into<String>) -> Result<(), DistributedIdError> {
    set_generator(DistributedIdGenerator::init_scoped(app_name.into()))
}

/// Initializes the process-global generator with an externally assigned worker ID.
///
/// The caller is responsible for keeping worker IDs unique. Values are encoded
/// into 13 bits, so prefer IDs no greater than [`generator::MAX_WORKER_ID`].
pub fn init_scoped_id_generator_with_id(id: u32, name: impl Into<String>) -> Result<(), DistributedIdError> {
    set_generator(DistributedIdGenerator::init_scoped_with_id(id, name.into()))
}

fn set_generator(generator: DistributedIdGenerator) -> Result<(), DistributedIdError> {
    DISTRIBUTED_ID_GENERATOR
        .set(Arc::new(Mutex::new(generator)))
        .map_err(|_| DistributedIdError::AlreadyInitialized)
}

#[cfg(all(feature = "distributed-id-etcd", not(target_family = "wasm")))]
/// Reserves a worker ID in etcd and initializes the process-global generator.
///
/// A background task keeps the generator synchronized if lease recovery
/// assigns a different worker ID. Call [`shutdown_id_generator`] during graceful
/// shutdown to revoke the lease promptly.
pub async fn init_id_generator(
    app_name: impl Into<String>,
    options: EtcdWorkerOptions,
) -> Result<(), DistributedIdError> {
    if DISTRIBUTED_ID_GENERATOR.get().is_some() {
        return Err(DistributedIdError::AlreadyInitialized);
    }

    let app_name = app_name.into();
    let (worker, guard) = etcd::register_worker(&app_name, options).await?;
    let generator = Arc::new(Mutex::new(DistributedIdGenerator::from_worker(worker)));

    DISTRIBUTED_ID_GENERATOR
        .set(generator.clone())
        .map_err(|_| DistributedIdError::AlreadyInitialized)?;

    let guard_clone = guard.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            if guard_clone.is_closed() {
                break;
            }
            let current_worker_id = guard_clone.worker_id().await;
            let mut generator = generator.lock().await;
            if generator.worker.distributed_id != current_worker_id as u64 {
                generator.worker.distributed_id = current_worker_id as u64;
            }
        }
    });

    WORKER_LEASE_GUARD.set(guard).map_err(|_| DistributedIdError::AlreadyInitialized)
}

#[cfg(all(feature = "distributed-id-etcd", not(target_family = "wasm")))]
/// Stops lease renewal and releases the etcd worker reservation.
///
/// The process-global generator cannot be initialized again afterward because
/// its storage is intentionally one-shot.
pub async fn shutdown_id_generator() -> Result<(), DistributedIdError> {
    if let Some(guard) = WORKER_LEASE_GUARD.get() {
        guard.shutdown().await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_conversion_matches_encoded_value() {
        let timestamp = EPOCH_SINCE + 1234;
        let id = 1234 << 23;
        assert_eq!(id_to_unix_timestamp(id), timestamp);
        assert_eq!(id_to_datetime(id).unwrap().timestamp_millis() as u64, timestamp);
    }
}
