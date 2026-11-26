//! Snowflake-style identifiers backed by a process-global generator.
//!
//! Initialize exactly once before generating IDs. [`init_scoped_id_generator`]
//! assigns the reserved process-local worker ID; use
//! [`init_scoped_id_generator_with_id`] when deployment already guarantees a
//! unique worker ID. With `distributed-id-etcd`, `init_id_generator` reserves
//! a worker ID under a renewable etcd lease.
//!
//! IDs contain a millisecond timestamp, a 13-bit worker ID, and a 10-bit
//! sequence. By default the timestamp comes from the wall clock. A scoped
//! generator can use future sequence slots via [`allow_ahead_of_time`].
//!
//! # Example
//!
//! ```
//! use hero::distributed_id::{allow_ahead_of_time, id_to_datetime, init_scoped_id_generator, try_gen_id};
//!
//! async fn create_id() -> u64 {
//!     allow_ahead_of_time(100).unwrap();
//!     init_scoped_id_generator("thumbnail-worker").unwrap();
//!     let id = try_gen_id().await.unwrap();
//!     assert!(id_to_datetime(id).is_ok());
//!     id
//! }
//! ```

use atomic::{AtomicGenerator, MAX_AHEAD_MS};
use chrono::{DateTime, Utc};
use generator::{DistributedIdError, MAX_WORKER_ID};
use std::sync::OnceLock;

mod atomic;

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

static DISTRIBUTED_ID_GENERATOR: OnceLock<AtomicGenerator> = OnceLock::new();
static AHEAD_OF_TIME_MS: OnceLock<u64> = OnceLock::new();
#[cfg(all(feature = "distributed-id-etcd", not(target_family = "wasm")))]
static WORKER_LEASE_GUARD: OnceLock<etcd::WorkerLeaseGuard> = OnceLock::new();

/// Sets the process-global scoped generator's maximum clock lead in milliseconds.
///
/// Call once before initializing the generator. `0` is the default and uses
/// only wall-clock timestamps. Values up to `8191` allow generation to use
/// future sequence slots before waiting. When reusing a scoped worker ID after
/// shutdown or restart, wait until the wall clock is past every timestamp that
/// the previous owner could have issued.
///
/// Returns an error if configuration is already set, initialization has fixed the limit,
/// or `milliseconds` exceeds `8191`. etcd-backed initialization requires `0`.
pub fn allow_ahead_of_time(milliseconds: u64) -> Result<(), DistributedIdError> {
    if milliseconds > MAX_AHEAD_MS {
        return Err(DistributedIdError::InvalidConfig(format!(
            "clock lead must be at most {MAX_AHEAD_MS} milliseconds"
        )));
    }

    AHEAD_OF_TIME_MS.set(milliseconds).map_err(|_| DistributedIdError::AlreadyInitialized)
}

/// Generates an ID from the initialized process-global generator.
///
/// Generation reserves sequence numbers atomically. When all slots
/// within the configured clock lead are in use, it waits for the wall clock to advance.
pub async fn try_gen_id() -> Result<u64, DistributedIdError> {
    DISTRIBUTED_ID_GENERATOR.get().ok_or(DistributedIdError::NotInitialized)?.next_id().await
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
    DISTRIBUTED_ID_GENERATOR.get().ok_or(DistributedIdError::NotInitialized)?.next_id_sync()
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

/// Converts an ID's encoded timestamp into UTC.
pub fn id_to_datetime(id: u64) -> Result<DateTime<Utc>, DistributedIdError> {
    let timestamp = id_to_unix_timestamp(id);
    DateTime::from_timestamp_millis(timestamp as i64).ok_or(DistributedIdError::InvalidTimestamp(id))
}

/// Initializes the process-global generator for a single-process deployment.
///
/// This mode uses the highest non-reserved worker slot. It must not be used by
/// multiple processes that can generate IDs for the same data set.
pub fn init_scoped_id_generator(app_name: impl Into<String>) -> Result<(), DistributedIdError> {
    let _app_name: String = app_name.into();
    set_scoped_generator(u64::from(MAX_WORKER_ID - 1))
}

/// Initializes the process-global generator with an externally assigned worker ID.
///
/// The caller is responsible for keeping worker IDs unique. Values are encoded
/// into 13 bits, so prefer IDs no greater than [`generator::MAX_WORKER_ID`].
pub fn init_scoped_id_generator_with_id(id: u32, name: impl Into<String>) -> Result<(), DistributedIdError> {
    let _name: String = name.into();
    set_scoped_generator(u64::from(id))
}

fn set_scoped_generator(worker_id: u64) -> Result<(), DistributedIdError> {
    let max_ahead_ms = *AHEAD_OF_TIME_MS.get_or_init(|| 0);
    DISTRIBUTED_ID_GENERATOR
        .set(AtomicGenerator::scoped(worker_id, max_ahead_ms))
        .map_err(|_| DistributedIdError::AlreadyInitialized)
}

#[cfg(all(feature = "distributed-id-etcd", not(target_family = "wasm")))]
/// Reserves a worker ID in etcd and initializes the process-global generator.
///
/// A background task keeps the generator synchronized if lease recovery
/// assigns a different worker ID. Call [`shutdown_id_generator`] during graceful
/// shutdown to revoke the lease promptly. This mode requires zero clock lead.
pub async fn init_id_generator(
    app_name: impl Into<String>,
    options: EtcdWorkerOptions,
) -> Result<(), DistributedIdError> {
    if DISTRIBUTED_ID_GENERATOR.get().is_some() {
        return Err(DistributedIdError::AlreadyInitialized);
    }
    if AHEAD_OF_TIME_MS.get().is_some_and(|limit| *limit != 0) {
        return Err(DistributedIdError::InvalidConfig(
            "clock lead requires a scoped worker ID".into(),
        ));
    }

    let app_name = app_name.into();
    let (worker, guard) = etcd::register_worker(&app_name, options).await?;
    if *AHEAD_OF_TIME_MS.get_or_init(|| 0) != 0 {
        guard.shutdown().await?;
        return Err(DistributedIdError::InvalidConfig(
            "clock lead requires a scoped worker ID".into(),
        ));
    }
    if DISTRIBUTED_ID_GENERATOR.set(AtomicGenerator::coordinated(worker.distributed_id)).is_err() {
        guard.shutdown().await?;
        return Err(DistributedIdError::AlreadyInitialized);
    }

    let guard_clone = guard.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            if guard_clone.is_closed() {
                break;
            }
            let current_worker_id = guard_clone.worker_id().await;
            if let Some(generator) = DISTRIBUTED_ID_GENERATOR.get() {
                generator.update_worker(current_worker_id);
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
