use chrono::{DateTime, TimeZone, Utc};
use once_cell::sync::Lazy;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

static ID_GENERATOR: Lazy<Mutex<IdGenerator>> = Lazy::new(|| Mutex::new(IdGenerator::new()));

struct IdGenerator {
    last_timestamp: u64,
    sequence: u16
}

impl IdGenerator {
    fn new() -> Self {
        Self {
            last_timestamp: 0,
            sequence: 0
        }
    }

    fn current_millis() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards").as_millis() as u64
    }

    fn next_id(&mut self) -> u64 {
        let now = Self::current_millis();
        if now == self.last_timestamp {
            self.sequence += 1;
            if self.sequence == 0 {
                // sequence overflow in same ms
                // block until next millisecond
                while Self::current_millis() <= self.last_timestamp {}
                self.last_timestamp = Self::current_millis();
                self.sequence = 0;
            }
        } else {
            self.last_timestamp = now;
            self.sequence = 0;
        }

        (self.last_timestamp << 16) | self.sequence as u64
    }
}

pub async fn gen_id() -> u64 {
    let mut generator = ID_GENERATOR.lock().await;
    generator.next_id()
}

pub fn id_to_date(id: u64) -> DateTime<Utc> {
    let timestamp_ms = id >> 16;
    Utc.timestamp_millis(timestamp_ms as i64)
}
