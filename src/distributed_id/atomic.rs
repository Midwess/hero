#[cfg(target_family = "wasm")]
use chrono::Utc;
use futures_timer::Delay;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::Duration;
#[cfg(not(target_family = "wasm"))]
use std::time::{SystemTime, UNIX_EPOCH};

use super::generator::{DistributedIdError, EPOCH_SINCE, MAX_ID_IN_MS, MAX_WORKER_ID};

const SEQUENCE_BITS: u32 = 10;
const TIMESTAMP_SHIFT: u32 = 23;
const MAX_SEQUENCE: u64 = MAX_ID_IN_MS - 1;
const MAX_TIMESTAMP: u64 = u64::MAX >> TIMESTAMP_SHIFT;
pub(super) const MAX_AHEAD_MS: u64 = (1 << (TIMESTAMP_SHIFT - SEQUENCE_BITS)) - 1;
const RETRY_DELAY: Duration = Duration::from_millis(1);

#[cfg(test)]
#[path = "../../tests/support/distributed_id_atomic.rs"]
mod tests;

enum Worker {
    Scoped {
        bits: u64,
        max_ahead_ms: u64,
    },
    #[cfg(all(feature = "distributed-id-etcd", not(target_family = "wasm")))]
    Coordinated,
}

pub(super) struct AtomicGenerator {
    worker: Worker,
    // The middle 13 bits track clock lead for scoped IDs or worker identity for etcd IDs.
    state: AtomicU64,
}

impl AtomicGenerator {
    pub(super) const fn scoped(worker_id: u64, max_ahead_ms: u64) -> Self {
        Self {
            worker: Worker::Scoped {
                bits: (worker_id & MAX_WORKER_ID as u64) << SEQUENCE_BITS,
                max_ahead_ms,
            },
            state: AtomicU64::new(0),
        }
    }

    #[cfg(all(feature = "distributed-id-etcd", not(target_family = "wasm")))]
    pub(super) fn coordinated(worker_id: u64) -> Self {
        let timestamp = wall_ms().unwrap_or_default().min(MAX_TIMESTAMP);
        Self {
            worker: Worker::Coordinated,
            state: AtomicU64::new((timestamp << TIMESTAMP_SHIFT) | (worker_id << SEQUENCE_BITS) | MAX_SEQUENCE),
        }
    }

    #[cfg(all(feature = "distributed-id-etcd", not(target_family = "wasm")))]
    pub(super) fn update_worker(&self, worker_id: u16) {
        let mask = u64::from(MAX_WORKER_ID) << SEQUENCE_BITS;
        let bits = u64::from(worker_id) << SEQUENCE_BITS;
        let mut previous = self.state.load(Relaxed);
        while previous & mask != bits {
            let previous_timestamp = previous >> TIMESTAMP_SHIFT;
            let timestamp = wall_ms().unwrap_or(previous_timestamp).clamp(previous_timestamp, MAX_TIMESTAMP);
            let next = (timestamp << TIMESTAMP_SHIFT) | bits | MAX_SEQUENCE;
            match self.state.compare_exchange_weak(previous, next, Relaxed, Relaxed) {
                Ok(_) => return,
                Err(actual) => previous = actual,
            }
        }
    }

    pub(super) async fn next_id(&self) -> Result<u64, DistributedIdError> {
        loop {
            if let Some(id) = self.try_next_id()? {
                return Ok(id);
            }
            Delay::new(RETRY_DELAY).await;
        }
    }

    #[cfg(not(target_family = "wasm"))]
    pub(super) fn next_id_sync(&self) -> Result<u64, DistributedIdError> {
        loop {
            if let Some(id) = self.try_next_id()? {
                return Ok(id);
            }
            std::thread::sleep(RETRY_DELAY);
        }
    }

    #[inline]
    fn try_next_id(&self) -> Result<Option<u64>, DistributedIdError> {
        self.reserve(wall_ms)
    }

    fn reserve(&self, clock: impl Fn() -> Result<u64, DistributedIdError>) -> Result<Option<u64>, DistributedIdError> {
        let mut previous = self.state.load(Relaxed);
        loop {
            // Sample after loading state so a concurrent reservation cannot look like clock rollback.
            let now = clock()?;
            let previous_timestamp = previous >> TIMESTAMP_SHIFT;
            let metadata = (previous >> SEQUENCE_BITS) & MAX_AHEAD_MS;
            let (worker_bits, previous_lead, max_ahead_ms) = match self.worker {
                Worker::Scoped { bits, max_ahead_ms } => (bits, metadata, max_ahead_ms),
                #[cfg(all(feature = "distributed-id-etcd", not(target_family = "wasm")))]
                Worker::Coordinated => (metadata << SEQUENCE_BITS, 0, 0),
            };
            let (timestamp, sequence) = if now > previous_timestamp {
                (now, 0)
            } else {
                let previous_wall_ms = previous_timestamp - previous_lead;
                if now < previous_wall_ms {
                    return Err(DistributedIdError::ClockMovedBackwards {
                        previous_ms: previous_wall_ms,
                        current_ms: now,
                    });
                }

                let sequence = (previous & MAX_SEQUENCE) + 1;
                (previous_timestamp + (sequence >> SEQUENCE_BITS), sequence & MAX_SEQUENCE)
            };

            if timestamp > MAX_TIMESTAMP {
                return Err(DistributedIdError::InvalidTimestamp(EPOCH_SINCE.saturating_add(timestamp)));
            }
            let lead = timestamp - now;
            if lead > max_ahead_ms {
                return Ok(None);
            }

            let timestamp_bits = timestamp << TIMESTAMP_SHIFT;
            let middle_bits = match self.worker {
                Worker::Scoped { .. } => lead << SEQUENCE_BITS,
                #[cfg(all(feature = "distributed-id-etcd", not(target_family = "wasm")))]
                Worker::Coordinated => worker_bits,
            };
            let next = timestamp_bits | middle_bits | sequence;
            match self.state.compare_exchange_weak(previous, next, Relaxed, Relaxed) {
                Ok(_) => return Ok(Some(timestamp_bits | worker_bits | sequence)),
                Err(actual) => previous = actual,
            }
        }
    }
}

#[cfg(not(target_family = "wasm"))]
fn wall_ms() -> Result<u64, DistributedIdError> {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| DistributedIdError::ClockBeforeEpoch)?
        .as_millis();
    let ms = u64::try_from(ms).map_err(|_| DistributedIdError::InvalidTimestamp(u64::MAX))?;
    ms.checked_sub(EPOCH_SINCE).ok_or(DistributedIdError::ClockBeforeEpoch)
}

#[cfg(target_family = "wasm")]
fn wall_ms() -> Result<u64, DistributedIdError> {
    u64::try_from(Utc::now().timestamp_millis())
        .ok()
        .and_then(|ms| ms.checked_sub(EPOCH_SINCE))
        .ok_or(DistributedIdError::ClockBeforeEpoch)
}
