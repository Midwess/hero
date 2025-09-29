use std::path::PathBuf;
use std::pin::Pin;
use async_stream::stream;
use async_zip::{Compression, ZipEntry, ZipEntryBuilder};
use async_zip::base::write::{EntryStreamWriter, ZipFileWriter};
use bytes::BytesMut;
use futures::Stream;
use anyhow::Result;
use chrono::Utc;
use n0_future::io::AsyncWriteExt;
use crate::local_storage::entry::FileEntry;
use crate::local_storage::stream::IOCursor;
use futures_util::AsyncReadExt;
use n0_future::StreamExt;
use crate::utils::stream::{duplex, DuplexReader, DuplexWriter};

impl FileEntry {
    pub fn zip_entry(&self, base_path: &PathBuf) -> ZipEntry {
        let name = self.relative_path(base_path).unwrap().to_string_lossy().to_string();
        ZipEntryBuilder::new(name.into(), Compression::Stored)
            .last_modification_date(chrono::Utc::now().into())
            .uncompressed_size(self.size)
            .compressed_size(self.size)
            .build()
    }
}

pub type ZipInputStream = Pin<Box<dyn Stream<Item = Result<Box<dyn IOCursor>>> + Send + Sync>>;

pub struct ZipStream {
    inputs: ZipInputStream,
    buffer: BytesMut,
    zip_entry: FileEntry,
    reader: DuplexReader,
    current_writer: Option<EntryStreamWriter<DuplexWriter>>,
    current_cursor: Option<Box<dyn IOCursor>>,
}

impl ZipStream {
    pub async fn new(inputs: Vec<Box<dyn IOCursor>>, path: PathBuf, max_chunk_size: usize) -> anyhow::Result<Self> {
        let mut total_size = 0u64;
        for input in inputs.iter() {
            let entry = input.entry().await?;
            total_size += entry.size;
        }

        let zip_entry = FileEntry {
            is_dir: false,
            modified_at: Utc::now().into(),
            size: total_size, // This will not correctly reflect the actual size of the zip file
            path: path.clone()
        };

        let inputs = stream! {
            for input in inputs {
                yield Ok(input);
            }
        };

        Self::new_from_stream(Box::pin(inputs), zip_entry, max_chunk_size).await
    }

    pub async fn new_from_stream(mut stream: ZipInputStream, zip_entry: FileEntry, max_chunk_size: usize) -> anyhow::Result<Self> {
        let (writer, reader) = duplex();
        let max_chunk_size = max_chunk_size.min(zip_entry.size as usize);
        let mut buffer = BytesMut::with_capacity(max_chunk_size);
        buffer.resize(max_chunk_size, 0u8);
        let zip_writer = ZipFileWriter::new(writer);
        let Some(Ok(first_cursor)) = stream.next().await else {
            return Err(anyhow::anyhow!("No entries to zip"));
        };

        let first_entry = first_cursor.entry().await?.zip_entry(&zip_entry.path);

        Ok(Self {
            zip_entry,
            inputs: stream,
            buffer,
            reader,
            current_cursor: Some(first_cursor),
            current_writer: Some(zip_writer.write_entry_stream(first_entry).await?),
        })
    }

    pub fn chunk_size(&self) -> usize {
        self.buffer.len()
    }
}

#[cfg_attr(not(target_family = "wasm"), async_trait::async_trait)]
#[cfg_attr(target_family = "wasm", async_trait::async_trait(?Send))]
impl IOCursor for ZipStream {
    async fn next(&mut self, max_read: Option<u64>) -> anyhow::Result<Option<&[u8]>> {
        if let Some(writer) = self.current_writer.as_mut() {
            writer.flush().await?;
        };

        let read_size = max_read.unwrap_or(self.chunk_size() as u64) as usize;
        let read_size = read_size.min(self.chunk_size());
        let read_buffer = &mut self.buffer.as_mut()[..read_size];
        if let Ok(amount) = self.reader.read(read_buffer).await {
            if amount > 0 {
                let bytes = &self.buffer[..amount];
                return Ok(Some(bytes))
            }
        }

        if let (Some(writer), Some(cursor)) = (self.current_writer.as_mut(), self.current_cursor.as_mut()) {
            if let Some(chunk) = cursor.next(max_read).await? {
                writer.write_all(&chunk).await?;
                return self.next(max_read).await
            }
        }

        let Some(writer) = self.current_writer.take() else {
            return Ok(None);
        };

        let zip_writer = writer.close().await?;
        self.current_cursor = match self.inputs.next().await {
            Some(Ok(t)) => Some(t),
            Some(Err(e)) => return Err(e),
            None => None,
        };

        if let Some(next_cursor) = self.current_cursor.as_mut() {
            let entry = next_cursor.entry().await?.zip_entry(&self.zip_entry.path);
            let new_entry_stream = zip_writer.write_entry_stream(entry).await?;
            self.current_writer = Some(new_entry_stream);
            return self.next(max_read).await;
        }

        zip_writer.close().await?;
        self.next(max_read).await
    }

    async fn entry(&self) -> anyhow::Result<FileEntry> {
        Ok(self.zip_entry.clone())
    }
}
