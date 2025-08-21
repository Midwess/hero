use anyhow::Result;
use bytes::Bytes;

#[cfg_attr(not(target_family = "wasm"), async_trait::async_trait)]
#[cfg_attr(target_family = "wasm", async_trait::async_trait(?Send))]
pub trait IOCursor: Send + Sync {
    async fn next(&mut self) -> Result<Option<bytes::Bytes>>;

    async fn total_size(&self) -> Result<u64>;

    async fn read_all(&mut self) -> Result<Bytes> {
        let mut data = vec![];
        while let Some(current) = self.next().await? {
            data.extend_from_slice(&current);
        }

        Ok(data.into())
    }
}
