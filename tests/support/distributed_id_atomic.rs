use super::*;

#[test]
fn scoped_reservation_uses_each_slot_up_to_the_configured_limit() {
    for limit in [0, 2, MAX_AHEAD_MS] {
        let generator = AtomicGenerator::scoped(42, limit);
        let now = 100;
        let reserve = |time| generator.reserve(|| Ok(time)).unwrap();
        let capacity = (limit + 1) * MAX_ID_IN_MS;
        for index in 0..capacity {
            let timestamp = now + index / MAX_ID_IN_MS;
            let sequence = index % MAX_ID_IN_MS;
            assert_eq!(reserve(now), Some((timestamp << 23) | (42 << 10) | sequence));
        }
        assert_eq!(reserve(now), None);
        assert_eq!(reserve(now + 1), Some(((now + limit + 1) << 23) | (42 << 10)));
    }
}

#[test]
fn scoped_reservation_distinguishes_clock_rollback_from_clock_lead() {
    let generator = AtomicGenerator::scoped(42, 2);
    for _ in 0..=MAX_ID_IN_MS {
        assert!(generator.reserve(|| Ok(100)).unwrap().is_some());
    }
    assert!(matches!(
        generator.reserve(|| Ok(99)),
        Err(DistributedIdError::ClockMovedBackwards {
            previous_ms: 100,
            current_ms: 99
        })
    ));
    assert_eq!(generator.reserve(|| Ok(100)).unwrap(), Some((101 << 23) | (42 << 10) | 1));
}

#[cfg(all(feature = "distributed-id-etcd", not(target_family = "wasm")))]
#[test]
fn coordinated_reservation_skips_the_worker_handoff_millisecond() {
    let generator = AtomicGenerator::coordinated(42);
    generator.update_worker(3);
    let handoff = generator.state.load(Relaxed) >> TIMESTAMP_SHIFT;
    assert_eq!(generator.reserve(|| Ok(handoff)).unwrap(), None);
    assert_eq!(
        generator.reserve(|| Ok(handoff + 1)).unwrap(),
        Some(((handoff + 1) << 23) | (3 << 10))
    );
}
