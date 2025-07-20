use anyhow::Result;

#[cfg_attr(not(target_family = "wasm"), async_trait::async_trait)]
#[cfg_attr(target_family = "wasm", async_trait::async_trait(?Send))]
pub trait IOCursor: Send + Sync {
    async fn next(&mut self) -> Result<Option<bytes::Bytes>>;
    async fn total_size(&self) -> Result<u64>;
}
