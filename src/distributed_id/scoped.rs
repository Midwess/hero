use std::error::Error;
use std::fmt;

use rand::Rng;

pub const REGION_CODE_MIN: i32 = 1000;
pub const REGION_CODE_MAX: i32 = 9222;
pub const SUBSCRIBER_MOD: i32 = 100_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionCodeError {
    OutOfRange { value: i32, min: i32, max: i32 },
}

impl fmt::Display for RegionCodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegionCodeError::OutOfRange { value, min, max } => {
                write!(f, "region_code {value} out of range [{min}, {max}]")
            }
        }
    }
}

impl Error for RegionCodeError {}

pub fn generate_phone_number(region_code: i32) -> Result<i64, RegionCodeError> {
    if !(REGION_CODE_MIN..=REGION_CODE_MAX).contains(&region_code) {
        return Err(RegionCodeError::OutOfRange {
            value: region_code,
            min: REGION_CODE_MIN,
            max: REGION_CODE_MAX,
        });
    }
    let subscriber = rand::rng().random_range(0..SUBSCRIBER_MOD);
    Ok(region_code as i64 * SUBSCRIBER_MOD as i64 + subscriber as i64)
}

pub fn format_phone_number(phone: i64) -> String {
    let region = phone / SUBSCRIBER_MOD as i64;
    let subscriber = phone % SUBSCRIBER_MOD as i64;
    let high = subscriber / 10_000;
    let low = subscriber % 10_000;
    format!("{region:04}-{high:04}-{low:04}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    use crate::distributed_id::DISTRIBUTED_ID_GENERATOR;

    #[test]
    fn rejects_out_of_range_region() {
        for v in [999, 9223, 0, -1, i32::MIN, i32::MAX] {
            let err = generate_phone_number(v).unwrap_err();
            assert_eq!(
                err,
                RegionCodeError::OutOfRange {
                    value: v,
                    min: REGION_CODE_MIN,
                    max: REGION_CODE_MAX
                }
            );
        }
    }

    #[test]
    fn accepts_in_range_region() {
        for v in [1000, 5555, 9222] {
            assert!(generate_phone_number(v).is_ok());
        }
    }

    #[test]
    fn region_prefix_round_trip() {
        for region in [REGION_CODE_MIN, 3548, 7721, REGION_CODE_MAX] {
            for _ in 0..50 {
                let phone = generate_phone_number(region).unwrap();
                assert_eq!(phone / SUBSCRIBER_MOD as i64, region as i64);
                let sub = phone % SUBSCRIBER_MOD as i64;
                assert!((0..SUBSCRIBER_MOD as i64).contains(&sub));
            }
        }
    }

    #[test]
    fn max_phone_within_i64() {
        let max_phone = REGION_CODE_MAX as i64 * SUBSCRIBER_MOD as i64 + (SUBSCRIBER_MOD as i64 - 1);
        assert!(max_phone < i64::MAX);
        assert_eq!(max_phone, 922_299_999_999);
    }

    #[test]
    fn distribution_sanity() {
        let mut seen = HashSet::new();
        for _ in 0..1000 {
            seen.insert(generate_phone_number(3548).unwrap());
        }
        assert!(seen.len() >= 990, "expected >=990 distinct phones, got {}", seen.len());
    }

    #[test]
    fn format_phone_number_canonical() {
        assert_eq!(format_phone_number(354_812_345_678), "3548-1234-5678");
        let max = REGION_CODE_MAX as i64 * SUBSCRIBER_MOD as i64 + (SUBSCRIBER_MOD as i64 - 1);
        let s = format_phone_number(max);
        assert_eq!(s.len(), 14);
        assert_eq!(s, "9222-9999-9999");
    }

    #[test]
    fn does_not_touch_global_generator() {
        let before = DISTRIBUTED_ID_GENERATOR.get().is_some();
        for _ in 0..100 {
            let _ = generate_phone_number(3548).unwrap();
        }
        let after = DISTRIBUTED_ID_GENERATOR.get().is_some();
        assert_eq!(before, after);
    }
}
