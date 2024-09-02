#[cfg(test)]
pub mod test {
    use std::time::Duration;

    use tokio::time::sleep;

    use crate::utils::pool::allocator::{PoolBuilder, PoolResource};
    use crate::utils::pool::request::PoolRequestBuilder;

    pub struct Db {}

    #[async_trait::async_trait]
    impl PoolResource<()> for Db {
        async fn new(param: &()) -> Self {
            Self {}
        }
    }

    #[tokio::test]
    pub async fn test_pool() {
        let builder = PoolBuilder::new(());
        let pool = builder.min_pool_size(1).max_pool_size(3).build::<Db>().await;

        let requester = PoolRequestBuilder::new().pool(pool).retreiving_timeout(Duration::new(10, 0)).build();

        let response = requester.retreive().await;

        assert_eq!(response.is_some(), true);
        let response = requester.retreive().await;

        let response = requester.retreive().await;

        let response = requester.retreive().await;

        let response = requester.retreive().await;

        let response = requester.retreive().await;
        drop(response);
        let response = requester.retreive().await;
        assert_eq!(response.is_some(), true);
        let response = requester.retreive().await;
        //let response = requester.retreive().await;
        assert_eq!(response.is_some(), true);
        drop(response);
        sleep(Duration::new(100, 0)).await;
    }
}
