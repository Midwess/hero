use bytes::{Bytes, BytesMut};
use crate::local_storage::entry::FileEntry;

#[cfg_attr(not(target_family = "wasm"), async_trait::async_trait)]
#[cfg_attr(target_family = "wasm", async_trait::async_trait(?Send))]
pub trait IOCursor: Send + Sync {
    async fn next(&mut self, max: Option<u64>) -> anyhow::Result<Option<&[u8]>>;

    async fn read_all(&mut self) -> anyhow::Result<Bytes> {
        let mut data = BytesMut::new();
        while let Some(current) = self.next(None).await? {
            data.extend_from_slice(current);
        }

        Ok(data.into())
    }

    async fn entry(&self) -> anyhow::Result<FileEntry>;

    async fn end(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}
