pub mod pool;
pub mod pool_allocator;
pub mod finally;

use std::time::{SystemTime, UNIX_EPOCH};

pub fn random_number_in_range(min: u32, max: u32) -> u32 {
    let start = SystemTime::now();
    let since_epoch = start.duration_since(UNIX_EPOCH).expect("Time went backwards");
    let millis = since_epoch.as_millis();

    let rng = (millis % (max as u128 - min as u128 + 1)) as u32 + min;
    rng
}
