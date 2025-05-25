use anyhow::Result;
use bytes::Bytes;
use flate2::write::GzEncoder;
use flate2::Compression;
use md5::Context;
use notify::event::{ModifyKind, RemoveKind};
use notify::{Config, Event, EventKind, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{vec_deque, VecDeque};
use std::future::Future;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Weak};
use std::time::{Duration, SystemTime};
use tokio::fs::{self, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{broadcast, Mutex, OnceCell};

#[derive(Debug, Clone, PartialEq)]
struct TarEntry {
    is_dir: bool,
    modified_at: std::time::SystemTime,
    size: u64,
    relative_path: PathBuf,
    absolute_path: Option<PathBuf>,
}

impl TarEntry {
    fn from_metadata(root_path: PathBuf, current_path: PathBuf) -> Result<Self> {
        let metadata = current_path.metadata()?;
        let relative_path = if root_path.eq(&current_path) { 
            PathBuf::from("") 
        } else {
            current_path.strip_prefix(&root_path)
                .map(|p| p.to_path_buf())
                .unwrap_or(current_path.clone())
        };

        Ok(Self {
            is_dir: metadata.is_dir(),
            modified_at: metadata.modified()?,
            size: if metadata.is_dir() { 0u64 } else { metadata.len() },
            relative_path,
            absolute_path: Some(current_path)
        })
    }

    fn from_tar_header(header: &tar::Header) -> Result<Self> {
        let relative_path = header.path()?.into_owned();

        let mtime = header.mtime()?;
        let modified_at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(mtime);

        Ok(Self {
            is_dir: header.entry_type().is_dir(),
            modified_at,
            size: header.size()?,
            relative_path,
            absolute_path: None
        })
    }

    // If return true, then we don't need to compress again
    fn is_tar_equal(&self, other: &Self) -> bool {
        self.is_dir == other.is_dir &&
        self.size == other.size &&
        self.relative_path == other.relative_path &&
        self.modified_at.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() == 
            other.modified_at.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
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

type IsCompleted = bool;

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

pub struct Folder {
    pub name: String,
    pub path: PathBuf,
    pub tar_process: Mutex<Option<(PathBuf, IsCompleted, Weak<broadcast::Sender<OnFlyCursorEvent>>)>>
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

        Ok(Self {
            name,
            path,
            tar_process: Default::default()
        })
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

    fn new_tar(file: std::fs::File) -> Result<tar::Builder<std::fs::File>> {
        Ok(tar::Builder::new(file))
    }

    async fn cursor_tar(&self, out_file_path: PathBuf, buffer_size: usize) -> Result<Box<dyn IOCursor>> {
        let mut process = self.tar_process.lock().await; 

        if process.is_none() {
            let mut out_file = File::new(None, out_file_path.clone()).await?.open().await?;

            let mut header_buffer = [0u8; 512];

            let (broadcast_tx, _) = broadcast::channel(1);
            let broadcast_tx = Arc::new(broadcast_tx);

            let path_clone = out_file_path.clone();

            let broadcast_tx_for_watcher = broadcast_tx.clone();
            let mut watcher = RecommendedWatcher::new(
                move |res: Result<Event, _>| match res {
                    Ok(event) => match event.kind {
                        EventKind::Modify(ModifyKind::Any) => {
                            let _ = broadcast_tx_for_watcher.send(OnFlyCursorEvent::Modified);
                        }
                        EventKind::Remove(RemoveKind::Any) => {
                            let _ = broadcast_tx_for_watcher.send(OnFlyCursorEvent::Removed);
                        }
                        _ => {}
                    },
                    Err(err) => {
                        let _ = broadcast_tx_for_watcher
                            .send(OnFlyCursorEvent::WriteFailed(format!("Watcher on event: {err:?}")));
                    }
                },
                Config::default().with_poll_interval(Duration::from_secs(8))
            )?;

            watcher.watch(&path_clone, RecursiveMode::NonRecursive)?;
            
            let broadcast_tx_clone = broadcast_tx.clone();
            let src_path = self.path.clone();
            let out_file_path_cloned = out_file_path.clone();
            let mut entries = vec![TarEntry::from_metadata(self.path.clone(), self.path.clone())?];

            // We try to restore the last tar progress by
            // parsing the current file if exist
            let mut safe_check_point = 0; 
            out_file.seek(SeekFrom::Start(0)).await?;
            while let Some(entry) = entries.pop() {
                if entry.is_dir {
                    if let Ok(mut dir_entries) = tokio::fs::read_dir(entry.absolute_path.as_ref().unwrap()).await {
                        while let Some(dir_entry) = dir_entries.next_entry().await? {
                            let path = dir_entry.path();
                            if let Ok(tar_entry) = TarEntry::from_metadata(src_path.clone(), path) {
                                entries.push(tar_entry);
                            }
                        }

                        entries.sort_by_key(|entry| entry.modified_at);
                    }

                    if entry.relative_path.as_os_str().is_empty() {
                        continue;
                    }
                }

                let old_entry = out_file.read_exact(&mut header_buffer).await.ok().and_then(|_| {
                    let mut header = tar::Header::new_old();
                    header.as_mut_bytes().copy_from_slice(&header_buffer);
                    let result = TarEntry::from_tar_header(&header);

                    result.ok()
                });

                if let Some(old_entry) = old_entry {
                    if old_entry.is_tar_equal(&entry) {
                        let data_size = (old_entry.size + 511) & !511;
                        if out_file.seek(std::io::SeekFrom::Current(data_size as i64)).await.is_ok() {
                            safe_check_point = out_file.stream_position().await?;
                            continue;
                        }
                    }
                };

                out_file.seek(SeekFrom::Start(safe_check_point)).await?;
                out_file.set_len(safe_check_point).await?;
                break;
            }

            let mut tar = Self::new_tar(out_file.into_std().await)?;
            tokio::task::spawn_blocking(move || {
                let _ = tar.get_mut().seek(SeekFrom::Start(safe_check_point));
                while let Some(entry) = entries.pop() {
                    if entry.is_dir {
                        if let Ok(dir_entries) = std::fs::read_dir(entry.absolute_path.as_ref().unwrap()) {
                            for dir_entry in dir_entries {
                                if let Ok(dir_entry) = dir_entry {
                                    let path = dir_entry.path();
                                    if let Ok(tar_entry) = TarEntry::from_metadata(src_path.clone(), path) {
                                        entries.push(tar_entry);
                                    }
                                }
                            }

                            entries.sort_by_key(|entry| entry.modified_at);
                        }

                        if entry.relative_path.as_os_str().is_empty() || entry.relative_path == PathBuf::from(".") {
                            continue;
                        }
                    }

                    if entry.is_dir {
                        if let Err(err) = tar.append_dir(entry.relative_path, entry.absolute_path.as_ref().unwrap()) {
                            let _ = std::fs::remove_file(&out_file_path_cloned);
                            let _ = broadcast_tx_clone.send(OnFlyCursorEvent::WriteFailed(format!("Failed to append directory: {err:?}")));
                            return;
                        }
                    }
                    else {
                        if let Err(err) = tar.append_path_with_name(entry.absolute_path.as_ref().unwrap(), entry.relative_path) {
                            let _ = std::fs::remove_file(&out_file_path_cloned);
                            let _ = broadcast_tx_clone.send(OnFlyCursorEvent::WriteFailed(format!("Failed to append file: {err:?}")));
                            return;
                        }
                    }
                }

                if let Err(err) = tar.get_mut().flush()
                    .and_then(|_| tar.get_mut().sync_all())
                {
                    let _ = std::fs::remove_file(&out_file_path_cloned);
                    let _ =
                        broadcast_tx_clone.send(OnFlyCursorEvent::WriteFailed(format!("Compression failed: {err:?}")));
                } else {
                    let _ = broadcast_tx_clone.send(OnFlyCursorEvent::WriteCompleted);
                }
            });

            *process = Some((out_file_path.clone(), false, Arc::downgrade(&broadcast_tx)));
        }

        let (out_file_path, is_completed, broadcast_tx) = process.as_ref().expect("Already set, cannot be undefined");

        if *is_completed {
            let file = File::existing(out_file_path).await?.cursor(0, buffer_size).await?;
            return Ok(file)
        }

        if let Some(broadcast) = broadcast_tx.upgrade() {
            let notify_rx = broadcast.subscribe();
            let cursor = OnFlyCursor::new(out_file_path.clone(), buffer_size, notify_rx).await?;
            return Ok(Box::new(cursor));
        }

        Err(anyhow::anyhow!("Cannot create a new cursor"))
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

#[derive(Debug, Clone)]
pub enum OnFlyCursorEvent {
    Modified,
    Removed,
    // Writer confirm that file is fully write
    WriteCompleted,
    // Writer confirm that the file is now invalid due to an error
    WriteFailed(String)
}

#[derive(Debug)]
pub struct OnFlyCursor {
    file: tokio::fs::File,
    position: u64,
    buffer: Vec<u8>,
    is_completd: bool,
    rx: broadcast::Receiver<OnFlyCursorEvent>
}

impl OnFlyCursor {
    pub async fn new(path: PathBuf, buffer_size: usize, rx: broadcast::Receiver<OnFlyCursorEvent>) -> Result<Self> {
        let file = fs::File::open(path.clone()).await?;
        Ok(Self {
            file,
            position: 0,
            buffer: vec![0; buffer_size],
            rx,
            is_completd: false
        })
    }
}

#[async_trait::async_trait]
impl IOCursor for OnFlyCursor {
    async fn next(&mut self) -> Result<Option<Bytes>> {
        loop {
            self.file.seek(std::io::SeekFrom::Start(self.position)).await?;
            let bytes_read = self.file.read(&mut self.buffer).await?;

            if bytes_read > 0 {
                self.position += bytes_read as u64;
                return Ok(Some(Bytes::copy_from_slice(&self.buffer[..bytes_read])));
            }

            if self.is_completd {
                return Ok(None)
            }

            match self.rx.recv().await {
                Ok(OnFlyCursorEvent::Modified) => continue,
                Ok(OnFlyCursorEvent::Removed) => return Err(anyhow::anyhow!("File removed")),
                Ok(OnFlyCursorEvent::WriteCompleted) => {
                    self.is_completd = true;
                    continue;
                }
                Ok(OnFlyCursorEvent::WriteFailed(e)) => return Err(anyhow::anyhow!(e)),
                Err(broadcast::error::RecvError::Closed) => return Ok(None),
                Err(broadcast::error::RecvError::Lagged(_)) => continue
            }
        }
    }
}

#[cfg(test)]
pub mod test {
    use anyhow::Result;
    use flate2::read::GzDecoder;
    use tokio_scoped::scoped;
    use std::fs;
    use std::io::Read;
    use std::path::Path;
    use tar::Archive;
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;

    use crate::local_storage::file_system::Folder;
    use crate::logger;

    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_tar() -> Result<()> {
        logger::setup();
        let source_folder = "/Users/tiendang/Projects/affinitypool";
        let output_file1 = "/Users/tiendang/Projects/affinitypool1.tar";
        let output_file2 = "/Users/tiendang/Projects/affinitypool2.tar";
        let output_file3 = "/Users/tiendang/Projects/affinitypool3.tar";

        let folder = Folder::new(source_folder).await?;
        
        // Create three concurrent tar cursors
        let mut cursor1 = folder.cursor_tar("/Users/tiendang/Projects/affinitypool_temp1.tar".into(), 1024 * 12).await?;
        let mut cursor2 = folder.cursor_tar("/Users/tiendang/Projects/affinitypool_temp2.tar".into(), 1024 * 12).await?;
        let mut cursor3 = folder.cursor_tar("/Users/tiendang/Projects/affinitypool_temp3.tar".into(), 1024 * 12).await?;

        tokio_scoped::scope(|s| {
            s.spawn(async {
                let mut out_file1 = File::create(output_file1).await.unwrap();
                loop {
                    let chunk = cursor1.next().await.unwrap();
                    match chunk {
                        Some(chunk) => {
                            let _ = out_file1.write_all(&chunk).await;
                        }
                        None => {
                            break;
                        }
                    }
                }
                let _ = out_file1.sync_all().await;
                println!("Compression 1 completed: {output_file1}");
            });

            s.spawn(async {
                let mut out_file2 = File::create(output_file2).await.unwrap();
                loop {
                    let chunk = cursor2.next().await.unwrap();
                    match chunk {
                        Some(chunk) => {
                            let _ = out_file2.write_all(&chunk).await;
                            out_file2.sync_all().await;
                        }
                        None => {
                            break;
                        }
                    }
                }
                let _ = out_file2.sync_all().await;
                println!("Compression 2 completed: {output_file2}");
            });

            s.spawn(async {
                let mut out_file3 = File::create(output_file3).await.unwrap();
                loop {
                    let chunk = cursor3.next().await.unwrap();
                    match chunk {
                        Some(chunk) => {
                            let _ = out_file3.write_all(&chunk).await;
                            out_file3.sync_all().await;
                        }
                        None => {
                            break;
                        }
                    }
                }
                let _ = out_file3.sync_all().await;
                println!("Compression 3 completed: {output_file3}");
            });
        });

        // Verify one of the archives by extracting
        let extract_path = "/tmp/affinitypool_extracted";
        if Path::new(extract_path).exists() {
            fs::remove_dir_all(extract_path)?;
        }
        fs::create_dir_all(extract_path)?;

        // Read tar file into memory
        let mut tar_file = fs::File::open(output_file1)?;
        let mut tar_data = Vec::new();
        tar_file.read_to_end(&mut tar_data)?;

        // Extract tar archive
        let mut archive = Archive::new(&tar_data[..]);
        archive.unpack(extract_path)?;

        println!("Extraction completed successfully");

        Ok(())
    }
}
