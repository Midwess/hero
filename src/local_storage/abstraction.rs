use anyhow::Result;

#[async_trait::async_trait]
pub trait IOCursor: Send + Sync {
    async fn next(&mut self) -> Result<Option<bytes::Bytes>>;
    async fn total_size(&self) -> Result<u64>;
}
