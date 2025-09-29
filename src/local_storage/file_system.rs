use anyhow::Result;
use bytes::BytesMut;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use rayon::prelude::*;
use async_stream::stream;
use tokio::fs::{self, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::spawn_blocking;
use walkdir::WalkDir;
use crate::local_storage::entry::FileEntry;
use crate::local_storage::stream::IOCursor;
use crate::local_storage::zip::ZipStream;

impl FileEntry {
    pub async fn existing(path: impl AsRef<Path>) -> Result<Self> {
        let path = PathBuf::from(path.as_ref());
        if !path.exists() {
            return Err(anyhow::anyhow!("File does not exist {:?}", path));
        }
        let metadata = path.metadata()?;
        let modified_at = metadata.modified()?;

        Ok(Self {
            is_dir: path.is_dir(),
            size: if path.is_dir() {
                0
            } else {
                path.metadata()?.len()
            },
            modified_at,
            path,
        })
    }

    pub async fn new(content: Option<Vec<u8>>, path: impl AsRef<Path>) -> Result<Self> {
        let path = PathBuf::from(path.as_ref());

        if path.is_dir() {
            return Err(anyhow::anyhow!("Path is a directory"));
        }

        if !path.parent().unwrap().try_exists().unwrap_or(false) {
            fs::create_dir_all(path.parent().unwrap()).await?;
        }

        if let Some(content) = content {
            if !path.exists() {
                fs::write(&path, &content).await?;
            }
        }

        if !path.exists() {
            fs::File::create(&path).await?;
        }

        Self::existing(path).await
    }

    pub async fn append(content: Vec<u8>, path: impl AsRef<Path>) -> Result<Self> {
        let path = PathBuf::from(path.as_ref());

        if path.is_dir() {
            return Err(anyhow::anyhow!("Path is a directory"));
        }

        if !path.parent().unwrap().try_exists().unwrap_or(false) {
            fs::create_dir_all(path.parent().unwrap()).await?;
        }

        let mut file = OpenOptions::new().create(true).append(true).open(&path).await?;

        file.write_all(&content).await?;

        Self::existing(path).await
    }

    pub async fn write(&mut self, content: Vec<u8>) -> Result<()> {
        let mut file = OpenOptions::new().create(true).append(true).open(&self.path).await?;

        file.write_all(&content).await?;
        Ok(())
    }

    pub async fn open(&self) -> Result<fs::File> {
        let file = OpenOptions::new().create(true).write(true).read(true).open(&self.path).await?;

        Ok(file)
    }

    pub async fn open_append(&self) -> Result<fs::File> {
        let file = OpenOptions::new().create(true).append(true).write(true).read(true).open(&self.path).await?;

        Ok(file)
    }

    pub async fn delete(self) -> Result<()> {
        fs::remove_file(&self.path).await?;
        Ok(())
    }

    pub async fn read(&self) -> Result<Vec<u8>> {
        Ok(fs::read(&self.path).await?)
    }

    pub async fn cursor(&self, buffer_size: usize) -> Result<Box<dyn IOCursor>> {
        Ok(Box::new(FileCursor::new(self.path.clone(), buffer_size).await?))
    }
}

#[derive(Debug, Clone)]
pub struct Folder {
    pub name: String,
    pub path: PathBuf
}

impl Folder {
    pub async fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = PathBuf::from(path.as_ref());
        if !path.exists() {
            fs::create_dir_all(&path).await?;
        }

        if path.is_file() {
            return Err(anyhow::anyhow!("Path is a file"));
        }

        let name = path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid folder name"))?
            .to_string_lossy()
            .into_owned();

        Ok(Self { name, path })
    }

    pub async fn delete(self) -> Result<()> {
        fs::remove_dir_all(&self.path).await?;
        Ok(())
    }

    #[cfg(feature = "zip")]
    pub async fn cursor(&self, chunk_size: usize) -> Result<Box<dyn IOCursor>> {
        let entry = FileEntry {
            is_dir: false,
            modified_at: SystemTime::now(),
            size: self.size().await,
            path: self.path.clone(),
        };

        let path = self.path.clone();
        let stream = stream! {
            let mut pending_paths = vec![path];

            while let Some(current_path) = pending_paths.pop() {
                if current_path.is_dir() {
                    let mut dir_entries = fs::read_dir(&current_path).await?;
                    while let Some(dir_entry) = dir_entries.next_entry().await? {
                        pending_paths.push(dir_entry.path());
                    }
                }
                else {
                    let cursor = FileCursor::new(current_path, chunk_size).await?;
                    yield Ok(Box::new(cursor) as Box<dyn IOCursor>);
                }
            }
        };

        let cursor = ZipStream::new_from_stream(Box::pin(stream), entry, chunk_size).await?;
        Ok(Box::new(cursor))
    }

    pub async fn size(&self) -> u64 {
        let path = self.path.clone();
        spawn_blocking(move || {
            WalkDir::new(&path)
                .into_iter()
                .par_bridge()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .filter_map(|e| e.metadata().map(|m| m.len()).ok())
                .sum()
        }).await.unwrap()
    }
}

#[derive(Debug)]
pub struct FileCursor {
    file: fs::File,
    path: PathBuf,
    position: usize,
    is_eof: bool,
    buffer: BytesMut
}

impl FileCursor {
    pub async fn new(path: PathBuf, buffer_size: usize) -> Result<Self> {
        let mut buffer = BytesMut::with_capacity(buffer_size);
        buffer.resize(buffer_size, 0u8);
        Ok(Self {
            file: fs::File::open(&path).await?,
            path,
            position: 0,
            is_eof: false,
            buffer
        })
    }

    pub fn is_eof(&self) -> bool {
        self.is_eof
    }
}

#[async_trait::async_trait]
impl IOCursor for FileCursor {
    async fn next(&mut self, max_read: Option<u64>) -> Result<Option<&[u8]>> {
        if self.is_eof {
            return Ok(None);
        }

        let read_size = max_read.unwrap_or(self.buffer.len() as u64) as usize;
        let read_size = read_size.min(self.buffer.len());
        let read_buffer = &mut self.buffer.as_mut()[..read_size];
        let bytes_read = self.file.read(read_buffer).await?;
        if bytes_read == 0 {
            self.is_eof = true;
            return Ok(None);
        }

        self.position += bytes_read;
        Ok(Some(&self.buffer[..bytes_read]))
    }

    async fn entry(&self) -> Result<FileEntry> {
        FileEntry::existing(self.path.clone()).await
    }
}
