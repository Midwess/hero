use anyhow::Result;
use md5::Context;
use std::future::Future;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::fs::{self, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{Mutex, OnceCell};
use tokio::task::spawn_blocking;

#[derive(Debug, Clone, PartialEq)]
struct TarEntry {
    is_dir: bool,
    is_file: bool,
    modified_at: std::time::SystemTime,
    size: u64,
    relative_path: PathBuf,
    absolute_path: Option<PathBuf>
}

impl TarEntry {
    fn from_metadata(root_path: PathBuf, current_path: PathBuf) -> Result<Self> {
        let metadata = current_path.metadata()?;
        let relative_path = if root_path.eq(&current_path) {
            PathBuf::from("")
        } else {
            current_path.strip_prefix(&root_path).map(|p| p.to_path_buf()).unwrap_or(current_path.clone())
        };

        Ok(Self {
            is_dir: metadata.is_dir(),
            is_file: metadata.is_file(),
            modified_at: metadata.modified()?,
            size: if metadata.is_dir() { 0u64 } else { metadata.len() },
            relative_path,
            absolute_path: Some(current_path)
        })
    }
}

#[derive(Debug, Clone)]
pub struct Metadata {
    pub size: u64,
    pub created_at: SystemTime,
    pub modified_at: SystemTime,
    pub is_hidden: bool,
    pub is_dir: bool,
    pub files_count: Option<usize>,
    pub folders_count: Option<usize>,
    pub total_size: Option<u64>,
    pub path: PathBuf,
    md5_cell: Arc<OnceCell<String>>
}

#[derive(Debug, Clone)]
pub struct File {
    pub name: String,
    pub path: PathBuf
}

#[async_trait::async_trait]
pub trait IOCursor: Send + Sync {
    async fn next(&mut self) -> Result<Option<bytes::Bytes>>;
}

impl Metadata {
    pub fn from_path(
        path: impl AsRef<Path> + Send + 'static
    ) -> Pin<Box<dyn Future<Output = Result<Self>> + Send + 'static>> {
        Box::pin(async move {
            let path = path.as_ref().to_path_buf();
            let metadata = fs::metadata(&path).await?;
            let is_dir = metadata.is_dir();

            let mut result = Self {
                path: path.clone(),
                size: metadata.len(),
                created_at: metadata.created()?,
                modified_at: metadata.modified()?,
                is_hidden: path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with("."))
                    .unwrap_or(false),
                is_dir,
                files_count: None,
                folders_count: None,
                total_size: None,
                md5_cell: Arc::new(OnceCell::new())
            };

            if is_dir {
                let mut entries = fs::read_dir(&path).await?;
                let mut files_count = 0;
                let mut folders_count = 0;
                let mut total_size = 0;

                while let Some(entry) = entries.next_entry().await? {
                    let entry_metadata = Self::from_path(entry.path()).await?;
                    if entry_metadata.is_dir {
                        folders_count += 1;
                        if let Some(size) = entry_metadata.total_size {
                            total_size += size;
                        }
                        if let Some(files) = entry_metadata.files_count {
                            files_count += files;
                        }
                        if let Some(folders) = entry_metadata.folders_count {
                            folders_count += folders;
                        }
                    } else {
                        files_count += 1;
                        total_size += entry_metadata.size;
                    }
                }

                result.files_count = Some(files_count);
                result.folders_count = Some(folders_count);
                result.total_size = Some(total_size);
            }

            Ok(result)
        })
    }

    pub fn md5(&self) -> Pin<Box<dyn Future<Output = Result<String>> + Send + '_>> {
        Box::pin(async move {
            self.md5_cell
                .get_or_try_init(|| async {
                    const CHUNK_SIZE: usize = 1024 * 1024; // 1MB chunks
                    let mut context = Context::new();

                    if self.is_dir {
                        let mut entries = fs::read_dir(&self.path).await?;
                        let mut entries_vec = Vec::new();

                        while let Some(entry) = entries.next_entry().await? {
                            entries_vec.push(entry);
                        }
                        entries_vec.sort_by_key(|e| e.file_name());

                        for entry in entries_vec {
                            let metadata = Self::from_path(entry.path()).await?;
                            let entry_md5 = metadata.md5().await?;
                            context.consume(entry_md5.as_bytes());
                        }
                    } else {
                        let mut file = fs::File::open(&self.path).await?;
                        let mut buffer = vec![0; CHUNK_SIZE];

                        loop {
                            let n = file.read(&mut buffer).await?;
                            if n == 0 {
                                break;
                            }
                            context.consume(&buffer[..n]);
                        }
                    }

                    Ok(format!("{:x}", context.compute()))
                })
                .await
                .cloned()
        })
    }
}

impl File {
    pub async fn existing(path: impl AsRef<Path>) -> Result<Self> {
        let path = PathBuf::from(path.as_ref());
        if !path.exists() {
            return Err(anyhow::anyhow!("File does not exist {:?}", path));
        }

        let name = path.file_name().ok_or_else(|| anyhow::anyhow!("Invalid file name"))?.to_string_lossy().into_owned();
        Ok(Self { name, path })
    }

    pub async fn new(content: Option<Vec<u8>>, path: impl AsRef<Path>) -> Result<Self> {
        let path = PathBuf::from(path.as_ref());
        let name = path.file_name().ok_or_else(|| anyhow::anyhow!("Invalid file name"))?.to_string_lossy().into_owned();

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

        Ok(Self { name, path })
    }

    pub async fn append(content: Vec<u8>, path: impl AsRef<Path>) -> Result<Self> {
        let path = PathBuf::from(path.as_ref());
        let name = path.file_name().ok_or_else(|| anyhow::anyhow!("Invalid file name"))?.to_string_lossy().into_owned();

        if path.is_dir() {
            return Err(anyhow::anyhow!("Path is a directory"));
        }

        if !path.parent().unwrap().try_exists().unwrap_or(false) {
            fs::create_dir_all(path.parent().unwrap()).await?;
        }

        let mut file = OpenOptions::new().create(true).append(true).open(&path).await?;

        file.write_all(&content).await?;

        Ok(Self { name, path })
    }

    pub async fn metadata(&self) -> Result<Metadata> {
        Metadata::from_path(self.path.clone()).await
    }

    pub async fn write(&mut self, content: Vec<u8>) -> Result<()> {
        let mut file = OpenOptions::new().create(true).append(true).open(&self.path).await?;

        file.write_all(&content).await?;
        Ok(())
    }

    pub async fn open(&self) -> Result<tokio::fs::File> {
        let file = OpenOptions::new().create(true).write(true).read(true).open(&self.path).await?;

        Ok(file)
    }

    pub async fn open_append(&self) -> Result<tokio::fs::File> {
        let file = OpenOptions::new().create(true).append(true).write(true).read(true).open(&self.path).await?;

        Ok(file)
    }

    pub async fn delete(self) -> Result<()> {
        fs::remove_file(&self.path).await?;
        Ok(())
    }

    pub async fn move_to(&mut self, new_path: impl AsRef<Path>) -> Result<()> {
        let new_path = PathBuf::from(new_path.as_ref());
        fs::rename(&self.path, &new_path).await?;
        self.path = new_path;
        self.name = self
            .path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid file name"))?
            .to_string_lossy()
            .into_owned();
        Ok(())
    }

    pub async fn copy_to(&self, new_path: impl AsRef<Path>) -> Result<File> {
        let new_path = PathBuf::from(new_path.as_ref());
        fs::copy(&self.path, &new_path).await?;

        Ok(File {
            name: new_path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid file name"))?
                .to_string_lossy()
                .into_owned(),
            path: new_path
        })
    }

    pub async fn read(&self) -> Result<Vec<u8>> {
        Ok(fs::read(&self.path).await?)
    }

    pub async fn cursor(&self, start_position: usize, buffer_size: usize) -> Result<Box<dyn IOCursor>> {
        let mut file = fs::File::open(&self.path).await?;

        if start_position > 0 {
            use tokio::io::AsyncSeekExt;
            file.seek(std::io::SeekFrom::Start(start_position as u64)).await?;
        }

        Ok(Box::new(FileCursor {
            file,
            position: start_position,
            is_eof: false,
            buffer: vec![0; buffer_size]
        }))
    }
}

pub struct ChunkedWriter {
    pub chunk_size: usize,
    pub buffer: Vec<u8>
}

impl ChunkedWriter {
    pub fn new(chunk_size: usize) -> Self {
        Self {
            chunk_size,
            buffer: Vec::new()
        }
    }

    pub fn take(&mut self) -> Vec<u8> {
        let chunk = self.buffer.drain(..self.chunk_size.min(self.buffer.len())).collect();
        chunk
    }

    pub fn take_all(&mut self) -> Vec<u8> {
        self.buffer.drain(..self.buffer.len()).collect()
    }
}

impl Write for ChunkedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct TarCursor {
    cursor: Arc<Mutex<TarCursorBlocking>>
}

impl TarCursor {
    pub async fn new(folder: Folder, chunk_size: usize) -> Result<Self> {
        Ok(Self {
            cursor: Arc::new(Mutex::new(TarCursorBlocking::new(folder, chunk_size)?))
        })
    }
}

#[async_trait::async_trait]
impl IOCursor for TarCursor {
    async fn next(&mut self) -> Result<Option<bytes::Bytes>> {
        let cursor = Arc::clone(&self.cursor);
        spawn_blocking(move || {
            let mut cursor = cursor.blocking_lock();
            cursor.next()
        })
        .await?
    }
}

pub struct TarCursorBlocking {
    folder: Folder,
    chunk_size: usize,
    pending_entries: Vec<TarEntry>,
    tar: Option<tar::Builder<ChunkedWriter>>,
    entry_writer: Option<(tar::EntryWriter<ChunkedWriter>, std::fs::File)>
}

impl TarCursorBlocking {
    pub fn new(folder: Folder, chunk_size: usize) -> Result<Self> {
        Ok(Self {
            pending_entries: vec![TarEntry::from_metadata(folder.path.clone(), folder.path.clone())?],
            folder,
            tar: Some(tar::Builder::new(ChunkedWriter::new(chunk_size))),
            entry_writer: None,
            chunk_size
        })
    }

    fn next(&mut self) -> Result<Option<bytes::Bytes>> {
        if let Some((writer, file)) = self.entry_writer.as_mut() {
            let mut write_bytes = vec![0; self.chunk_size];
            let read_bytes = file.read(&mut write_bytes)?;
            if read_bytes > 0 {
                writer.write_all(&write_bytes[..read_bytes]).unwrap();
                return Ok(Some(bytes::Bytes::from(writer.get_mut().take())));
            }
        }

        if let Some((writer, _)) = self.entry_writer.take() {
            self.tar = Some(writer.finish()?);
            return self.next();
        }

        if self.tar.is_none() {
            return Ok(None);
        }

        if let Some(entry) = self.pending_entries.pop() {
            if entry.is_dir {
                let mut dir_entries = std::fs::read_dir(entry.absolute_path.as_ref().unwrap()).unwrap();
                while let Some(Ok(dir_entry)) = dir_entries.next() {
                    let entry_path = dir_entry.path();
                    let tar_entry = TarEntry::from_metadata(self.folder.path.clone(), entry_path)?;
                    self.pending_entries.push(tar_entry);
                }

                if entry.relative_path.as_os_str().is_empty() {
                    return self.next();
                }

                let relative_path = entry.relative_path.clone();
                let absolute_path = entry.absolute_path.clone();
                self.tar.as_mut().unwrap().append_dir(relative_path, absolute_path.as_ref().unwrap())?;

                return Ok(Some(bytes::Bytes::from(self.tar.as_mut().unwrap().get_mut().take())));
            } else if entry.is_file {
                let mut header = tar::Header::new_gnu();
                let metadata = entry.absolute_path.as_ref().unwrap().metadata()?;
                header.set_metadata_in_mode(&metadata, tar::HeaderMode::Complete);
                let writer = self.tar.take().unwrap().into_writer(header, entry.relative_path.clone())?;
                let file = std::fs::File::open(entry.absolute_path.as_ref().unwrap())?;
                self.entry_writer = Some((writer, file));

                return self.next();
            }
        }

        if let Some(mut tar) = self.tar.take() {
            tar.finish()?;
            return Ok(Some(bytes::Bytes::from(tar.get_mut().take_all())));
        }

        Ok(None)
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

    pub async fn metadata(&self) -> Result<Metadata> {
        Metadata::from_path(self.path.clone()).await
    }

    pub async fn calculate_total_size(&self) -> Result<u64> {
        Ok(self.metadata().await?.total_size.unwrap_or(0))
    }

    pub async fn delete(self) -> Result<()> {
        fs::remove_dir_all(&self.path).await?;
        Ok(())
    }

    pub async fn move_to(&mut self, new_path: impl AsRef<Path>) -> Result<()> {
        let new_path = PathBuf::from(new_path.as_ref());
        fs::rename(&self.path, &new_path).await?;
        self.path = new_path;
        self.name = self
            .path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid folder name"))?
            .to_string_lossy()
            .into_owned();
        Ok(())
    }

    pub async fn cursor(&self, chunk_size: usize) -> Result<Box<dyn IOCursor>> {
        let cursor = TarCursor::new(self.clone(), chunk_size).await?;
        Ok(Box::new(cursor))
    }

    pub async fn calculate_tar_size(&self) -> Result<u64> {
        let mut total_size = 0u64;
        let mut pending_entries = vec![TarEntry::from_metadata(self.path.clone(), self.path.clone())?];

        while let Some(entry) = pending_entries.pop() {
            total_size += 512;

            if entry.is_dir {
                let mut dir_entries = fs::read_dir(entry.absolute_path.as_ref().unwrap()).await?;
                while let Some(dir_entry) = dir_entries.next_entry().await? {
                    let entry_path = dir_entry.path();
                    let tar_entry = TarEntry::from_metadata(self.path.clone(), entry_path)?;
                    pending_entries.push(tar_entry);
                }
            } else if entry.is_file {
                let file_size = entry.size;
                total_size += file_size.div_ceil(512) * 512;
            }
        }

        total_size += 1024;
        // We substract the size of the root folder since we are not calculate it
        total_size -= 512;

        Ok(total_size)
    }
}

#[derive(Debug)]
pub struct FileCursor {
    file: tokio::fs::File,
    position: usize,
    is_eof: bool,
    buffer: Vec<u8> // Reusable buffer
}

impl FileCursor {
    pub fn new(file: tokio::fs::File, size: usize) -> Self {
        Self {
            file,
            position: 0,
            is_eof: false,
            buffer: vec![0; size]
        }
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn is_eof(&self) -> bool {
        self.is_eof
    }
}

#[async_trait::async_trait]
impl IOCursor for FileCursor {
    async fn next(&mut self) -> Result<Option<bytes::Bytes>> {
        if self.is_eof {
            return Ok(None);
        }

        let bytes_read = self.file.read(&mut self.buffer).await?;
        if bytes_read == 0 {
            self.is_eof = true;
            return Ok(None);
        }

        self.position += bytes_read;
        Ok(Some(bytes::Bytes::copy_from_slice(&self.buffer[..bytes_read])))
    }
}
