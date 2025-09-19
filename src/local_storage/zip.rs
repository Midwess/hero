use std::path::PathBuf;
use async_zip::{Compression, ZipEntry, ZipEntryBuilder};
use async_zip::base::write::{EntryStreamWriter, ZipFileWriter};
use bytes::BytesMut;
use n0_future::io::AsyncWriteExt;
use crate::local_storage::entry::FileEntry;
use crate::local_storage::stream::IOCursor;
use futures_util::AsyncReadExt;
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

pub struct ZipStream {
    inputs: Vec<Box<dyn IOCursor>>,
    buffer: BytesMut,
    zip_entry: FileEntry,
    reader: DuplexReader,
    current_writer: Option<EntryStreamWriter<DuplexWriter>>,
    current_index: usize,

    // I don't know why this is needed,
    // but it's required to stop the writer from being dropped
    // before all the data is fully retrieved.
    _writer: Option<DuplexWriter>
}

impl ZipStream {
    pub async fn new(inputs: Vec<Box<dyn IOCursor>>, path: PathBuf, max_chunk_size: usize) -> anyhow::Result<Self> {
        let (writer, reader) = duplex();
        let mut total_size = 0u64;
        for input in inputs.iter() {
            let entry = input.entry().await?;
            total_size += entry.size;
        }

        let Some(first_entry) = inputs.first() else {
            return Err(anyhow::anyhow!("No entries to zip"));
        };

        let first_entry = first_entry.entry().await?.zip_entry(&path);
        let zip_entry = FileEntry {
            is_dir: false,
            size: total_size, // This will not correctly reflect the actual size of the zip file
            modified_at: chrono::Utc::now().into(),
            path: path.clone()
        };

        let zip_writer = ZipFileWriter::new(writer);
        let max_chunk_size = max_chunk_size.min(zip_entry.size as usize);
        let mut buffer = BytesMut::with_capacity(max_chunk_size);
        buffer.resize(max_chunk_size, 0u8);

        Ok(Self {
            reader,
            current_writer: Some(zip_writer.write_entry_stream(first_entry).await?),
            zip_entry,
            inputs,
            current_index: 0,
            buffer,
            _writer: None
        })
    }

    pub fn chunk_size(&self) -> usize {
        self.buffer.capacity()
    }
}

#[async_trait::async_trait]
impl IOCursor for ZipStream {
    async fn next(&mut self, max_read: Option<u64>) -> anyhow::Result<Option<&[u8]>> {
        if let Some(writer) = self.current_writer.as_mut() {
            writer.flush().await?;
        };

        let read_size = max_read.unwrap_or(self.chunk_size() as u64) as usize;
        let read_buffer = &mut self.buffer.as_mut()[..read_size];
        if let Ok(amount) = self.reader.read(read_buffer).await {
            if amount > 0 {
                let bytes = &self.buffer[..amount];
                return Ok(Some(bytes))
            }
        }

        if let (Some(writer), Some(cursor)) = (self.current_writer.as_mut(), &mut self.inputs.get_mut(self.current_index)) {
            if let Some(chunk) = cursor.next(max_read).await? {
                writer.write_all(&chunk).await?;
                return self.next(max_read).await
            }
        }

        let Some(writer) = self.current_writer.take() else {
            return Ok(None);
        };

        let zip_writer = writer.close().await?;
        self.current_index += 1;

        if let Some(next_cursor) = self.inputs.get(self.current_index) {
            let entry = next_cursor.entry().await?.zip_entry(&self.zip_entry.path);
            let new_entry_stream = zip_writer.write_entry_stream(entry).await?;
            self.current_writer = Some(new_entry_stream);
            return self.next(max_read).await;
        }

        let finalize = zip_writer.close().await?;
        self._writer = Some(finalize);
        self.next(max_read).await
    }

    async fn entry(&self) -> anyhow::Result<FileEntry> {
        Ok(self.zip_entry.clone())
    }
}

#[cfg(all(feature = "zip"))]
#[cfg(feature="local-storage")]
#[cfg(test)]
mod tests {
    use crate::local_storage::file_system::Folder;
    use std::collections::HashMap;
    use tokio::fs;
    use async_zip::base::read::mem::ZipFileReader;
    use futures_util::io::AsyncReadExt;
    use crate::logger::setup;

    #[tokio::test]
    async fn test_zip_compress_and_decompress() {
        setup();
        let source_folder = std::path::PathBuf::from("src");

        let folder = Folder::new(&source_folder).await.unwrap();

        let entries = folder.collect_all_entries().await.unwrap();

        let mut expected_files = HashMap::new();
        let mut total_file_size = 0;
        for entry in &entries {
            let relative_path = entry.relative_path(&source_folder).unwrap();
            let relative_path_str = relative_path.to_string_lossy().replace("\\", "/");
            let content = fs::read(&entry.path).await.unwrap();
            total_file_size += content.len() as u64;
            expected_files.insert(relative_path_str, content);
        }

        let chunk_size = 1096;
        let mut zip_cursor = folder.cursor(chunk_size).await.unwrap();

        let mut zip_data = Vec::new();
        while let Some(chunk) = zip_cursor.next(None).await.unwrap() {
            zip_data.extend_from_slice(&chunk);
        }

        assert!(!zip_data.is_empty(), "Zip data should not be empty");
        let zip_entry = zip_cursor.entry().await.unwrap();

        assert!(!zip_entry.is_dir, "Zip entry should not be marked as directory");
        assert!(zip_entry.size > 0, "Zip entry size should be greater than 0");
        if let Err(e) = decompress_and_verify(&zip_data, &expected_files).await {
            panic!("Zip decompression failed: {e:?}");
        }
    }

    async fn decompress_and_verify(
        zip_data: &[u8],
        expected_files: &HashMap<String, Vec<u8>>
    ) -> anyhow::Result<()> {
        let zip_reader = ZipFileReader::new(zip_data.to_vec()).await?;

        let entries = zip_reader.file().entries();
        log::info!("Zip contains {} {:?} files", entries.len(), entries.iter().map(|e| {
            let name = e.filename().as_str().unwrap();
            let size = e.uncompressed_size() as usize;
            (name, size)
        }).collect::<Vec<_>>());

        assert!(!entries.is_empty(), "Zip should contain at least one file");
        assert_eq!(entries.len(), expected_files.len(), "Number of files in zip should match the expected number of files");

        for (i, entry) in entries.iter().enumerate() {
            let entry_name = entry.filename().as_str()?;

            let expected_content = expected_files.get(entry_name)
                .ok_or_else(|| anyhow::anyhow!("Unexpected file in zip: {}", entry_name))?;

            let mut entry_reader = zip_reader.reader_with_entry(i).await?;
            let mut actual_content = Vec::new();
            entry_reader.read_to_end(&mut actual_content).await?;

            assert_eq!(
                &actual_content, expected_content,
                "Content mismatch for file: {}", entry_name
            );

            assert_eq!(
                entry.uncompressed_size() as usize, expected_content.len(),
                "Size mismatch for file: {}", entry_name
            );
        }

        Ok(())
    }
}
