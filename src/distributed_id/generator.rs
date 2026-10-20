//! Low-level Snowflake identifier generation.

use chrono::{DateTime, Utc};
use futures_timer::Delay;
use std::time::Duration;
use thiserror::Error;

/// Unix timestamp in milliseconds used as the ID format's custom epoch.
///
/// The value represents `2024-12-29T00:00:00Z`.
pub const EPOCH_SINCE: u64 = 1735405200000; // 2024-12-29T00:00:00Z

/// Number of sequence values available to one worker per millisecond.
pub const MAX_ID_IN_MS: u64 = 1024;

/// Largest worker ID representable by the 13-bit worker field.
pub const MAX_WORKER_ID: u16 = (1 << 13) - 1;

#[derive(Error, Debug)]
/// Failures from generator initialization, coordination, or clock handling.
pub enum DistributedIdError {
    /// No process-global generator has been installed.
    #[error("distributed ID generator is not initialized")]
    NotInitialized,
    /// The process-global generator or lease guard was initialized twice.
    #[error("distributed ID generator is already initialized")]
    AlreadyInitialized,
    /// Coordinator settings cannot produce a valid etcd lease.
    #[error("Invalid coordinator configuration: {0}")]
    InvalidConfig(String),
    /// An etcd operation failed.
    #[error("Coordinator error: {0}")]
    Coordinator(String),
    /// Every representable worker ID is already reserved.
    #[error("Worker id space exhausted in namespace {0}")]
    WorkerIdsExhausted(String),
    /// An ID encoded a timestamp that `chrono` cannot represent.
    #[error("invalid timestamp encoded in distributed ID {0}")]
    InvalidTimestamp(u64),
    /// The system clock is earlier than Hero's custom epoch.
    #[error("system clock is before the distributed ID epoch")]
    ClockBeforeEpoch,
    /// The clock regressed after at least one ID had been generated.
    #[error("system clock moved backwards from {previous_ms}ms to {current_ms}ms since the epoch")]
    ClockMovedBackwards {
        /// Previously observed milliseconds since [`EPOCH_SINCE`].
        previous_ms: u64,
        /// Current milliseconds since [`EPOCH_SINCE`].
        current_ms: u64,
    },
}

#[derive(Debug, Clone)]
/// Identity and creation metadata for one generator worker.
pub struct Worker {
    /// Worker ID encoded into generated IDs.
    pub distributed_id: u64,
    /// Human-readable application or worker name.
    pub name: String,
    /// Time at which this worker record was created.
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
/// Stateful generator for one worker.
///
/// Prefer the process-level helpers in [`crate::distributed_id`] unless a
/// caller needs to manage generator state explicitly.
pub struct DistributedIdGenerator {
    /// Worker metadata used for the 13-bit worker field.
    pub worker: Worker,
    /// Last emitted millisecond offset from [`EPOCH_SINCE`].
    pub last_timestamp: u64,
    /// Sequence counter used within [`DistributedIdGenerator::last_timestamp`].
    pub sequence: u64,
}

impl DistributedIdGenerator {
    /// Starts a generator from an allocated worker record.
    pub fn from_worker(worker: Worker) -> Self {
        Self {
            worker,
            last_timestamp: 0,
            sequence: 0,
        }
    }

    /// Starts a process-local generator using Hero's reserved scoped worker ID.
    pub fn init_scoped(app_name: String) -> Self {
        Self {
            worker: Worker {
                distributed_id: (MAX_WORKER_ID - 1) as u64,
                name: app_name.clone(),
                created_at: Utc::now(),
            },
            last_timestamp: 0,
            sequence: 0,
        }
    }

    /// Starts a process-local generator with a caller-assigned worker ID.
    pub fn init_scoped_with_id(id: u32, name: String) -> Self {
        Self {
            worker: Worker {
                distributed_id: id as u64,
                name: name.clone(),
                created_at: Utc::now(),
            },
            last_timestamp: 0,
            sequence: 0,
        }
    }

    /// Generates the next monotonically increasing ID for this worker.
    ///
    /// The bit layout is 41 bits of milliseconds since [`EPOCH_SINCE`], 13
    /// bits of worker ID, and 10 bits of sequence. Sequence exhaustion waits
    /// for the next millisecond. Clock regression is returned as an error rather
    /// than risking an out-of-order or duplicate ID.
    pub async fn next_id(&mut self) -> Result<u64, DistributedIdError> {
        let current_ms =
            u64::try_from(Utc::now().timestamp_millis()).map_err(|_| DistributedIdError::ClockBeforeEpoch)?;
        let mut timestamp = current_ms.checked_sub(EPOCH_SINCE).ok_or(DistributedIdError::ClockBeforeEpoch)?;

        if timestamp < self.last_timestamp {
            return Err(DistributedIdError::ClockMovedBackwards {
                previous_ms: self.last_timestamp,
                current_ms: timestamp,
            });
        }

        if timestamp == self.last_timestamp {
            self.sequence += 1;

            if self.sequence >= MAX_ID_IN_MS {
                // Wait until we move to the next millisec
                while timestamp == self.last_timestamp {
                    Delay::new(Duration::from_millis(1)).await;
                    let new_ms = u64::try_from(Utc::now().timestamp_millis())
                        .map_err(|_| DistributedIdError::ClockBeforeEpoch)?
                        .checked_sub(EPOCH_SINCE)
                        .ok_or(DistributedIdError::ClockBeforeEpoch)?;
                    if new_ms > timestamp {
                        timestamp = new_ms;
                        break;
                    }
                }

                self.sequence = 0;
                self.last_timestamp = timestamp;
            }
        } else {
            self.sequence = 0;
            self.last_timestamp = timestamp;
        }

        // Construct the 64-bit ID
        // 41 bits of timestamp | 13 bits of worker | 10 bits of sequence
        let timestamp_41 = timestamp & ((1 << 41) - 1);
        let worker_13 = self.worker.distributed_id & ((1 << 13) - 1);
        let seq_10 = self.sequence & ((1 << 10) - 1);

        let id = (timestamp_41 << 23) | (worker_13 << 10) | seq_10;

        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_next_id_increments() -> Result<(), Box<dyn std::error::Error>> {
        let worker = Worker {
            distributed_id: 42,
            name: "test_worker".to_string(),
            created_at: Utc::now(),
        };

        let mut generator = DistributedIdGenerator {
            worker,
            last_timestamp: 0,
            sequence: 0,
        };

        let first_id = generator.next_id().await?;
        let second_id = generator.next_id().await?;

        assert!(second_id > first_id, "second_id should be greater than first_id");

        let mut prev_id = second_id;
        for _ in 0..10 {
            let next = generator.next_id().await?;
            assert!(next > prev_id, "IDs should be strictly increasing");
            prev_id = next;
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_sequence_rollover() -> Result<(), Box<dyn std::error::Error>> {
        let worker = Worker {
            distributed_id: 1,
            name: "rollover_test".to_string(),
            created_at: Utc::now(),
        };

        let mut generator = DistributedIdGenerator {
            worker,
            last_timestamp: 1000,       // pretend we're at "second 1000"
            sequence: MAX_ID_IN_MS - 1, // about to overflow
        };

        let first_id = generator.next_id().await?;
        assert_eq!(generator.sequence, 0, "sequence should have reset to 0");
        assert!(generator.last_timestamp >= 1000, "timestamp should move forward if we waited");

        let second_id = generator.next_id().await?;
        assert!(second_id > first_id, "IDs should keep increasing even after rollover");

        Ok(())
    }
}
