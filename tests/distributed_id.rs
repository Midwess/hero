#![cfg(all(feature = "distributed-id", not(target_family = "wasm")))]

use hero::distributed_id::{
    allow_ahead_of_time, id_to_unix_timestamp, init_scoped_id_generator_with_id, try_gen_id, try_gen_id_sync,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_async_and_sync_calls_share_one_worker_sequence() {
    allow_ahead_of_time(100).unwrap();
    assert!(allow_ahead_of_time(60).is_err());
    init_scoped_id_generator_with_id(42, "test").unwrap();
    assert!(allow_ahead_of_time(0).is_err());

    let first = try_gen_id_sync().unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..4 {
        tasks.spawn(async {
            let mut ids = Vec::with_capacity(4_096);
            for _ in 0..4_096 {
                ids.push(try_gen_id().await.unwrap());
            }
            assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
            ids
        });
    }
    tasks.spawn_blocking(|| (0..4_096).map(|_| try_gen_id_sync().unwrap()).collect::<Vec<_>>());

    let mut ids = vec![first];
    while let Some(task) = tasks.join_next().await {
        ids.extend(task.unwrap());
    }
    let last = try_gen_id_sync().unwrap();
    ids.push(last);
    ids.sort_unstable();
    assert_eq!(ids.first(), Some(&first));
    assert_eq!(ids.last(), Some(&last));
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(ids.iter().all(|id| (id >> 10) & 8191 == 42));
    assert!(id_to_unix_timestamp(last) >= id_to_unix_timestamp(first));
}
