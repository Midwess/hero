use anyhow::Result;
use md5::Context;
use std::future::Future;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::fs;
use tokio::io::AsyncReadExt;
use tokio::sync::OnceCell;
use zip::write::FileOptions;
use zip::ZipWriter;

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

#[derive(Debug, Clone)]
pub struct Folder {
    pub name: String,
    pub path: PathBuf
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
                .map(|s| s.clone())
        })
    }
}

impl File {
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

    pub async fn metadata(&self) -> Result<Metadata> {
        Metadata::from_path(self.path.clone()).await
    }

    pub async fn write(&mut self, content: Vec<u8>) -> Result<()> {
        fs::write(&self.path, content).await?;
        Ok(())
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

    pub async fn zip(&self, out_path: impl AsRef<Path>) -> Result<File> {
        let out_path = PathBuf::from(out_path.as_ref());
        let file = fs::File::create(&out_path).await?.into_std().await;
        let content = fs::read(&self.path).await?;
        let name = self.name.clone();

        // Spawn blocking task for ZIP operations
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut zip = ZipWriter::new(file);
            let options: FileOptions<'_, ()> =
                FileOptions::default().compression_method(zip::CompressionMethod::ZSTD).unix_permissions(0o755);

            zip.start_file(&name, options)?;
            zip.write_all(&content)?;
            zip.finish()?;
            Ok(())
        })
        .await??;

        File::new(None, out_path).await
    }
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

    pub async fn copy_to(&self, new_path: impl AsRef<Path>) -> Result<Folder> {
        let new_path = PathBuf::from(new_path.as_ref());
        copy_dir_all(&self.path, &new_path).await?;
        Folder::new(new_path).await
    }

    pub async fn add_file(&self, file: File, relative_path: impl AsRef<Path>) -> Result<File> {
        let target_path = self.path.join(relative_path);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        file.copy_to(target_path).await
    }

    pub async fn list_files(&self) -> Result<Vec<File>> {
        let mut files = Vec::new();
        let mut entries = fs::read_dir(&self.path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let metadata = entry.metadata().await?;
            if metadata.is_file() {
                files.push(File {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    path: entry.path()
                });
            }
        }

        Ok(files)
    }

    pub async fn list_folders(&self) -> Result<Vec<Folder>> {
        let mut folders = Vec::new();
        let mut entries = fs::read_dir(&self.path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let metadata = entry.metadata().await?;
            if metadata.is_dir() {
                folders.push(Folder {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    path: entry.path()
                });
            }
        }

        Ok(folders)
    }

    pub async fn zip(&self, out_path: impl AsRef<Path>) -> Result<File> {
        let out_path = PathBuf::from(out_path.as_ref());
        let file = fs::File::create(&out_path).await?.into_std().await;
        let base_path = self.path.clone();

        // First collect all file contents and paths
        fn collect_entries(
            path: PathBuf,
            base: &Path
        ) -> Pin<Box<dyn Future<Output = Result<Vec<(String, Option<Vec<u8>>)>>> + Send + '_>> {
            Box::pin(async move {
                let mut result = Vec::new();
                let mut dir = fs::read_dir(&path).await?;

                while let Some(entry) = dir.next_entry().await? {
                    let path = entry.path();
                    let relative_path = path.strip_prefix(base)?.to_string_lossy().into_owned();

                    if entry.metadata().await?.is_dir() {
                        result.push((relative_path + "/", None));
                        result.extend(collect_entries(path, base).await?);
                    } else {
                        let content = fs::read(&path).await?;
                        result.push((relative_path, Some(content)));
                    }
                }
                Ok(result)
            })
        }

        let entries = collect_entries(self.path.clone(), &base_path).await?;

        // Spawn blocking task for ZIP operations
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut zip = ZipWriter::new(file);
            let options: FileOptions<'_, ()> =
                FileOptions::default().compression_method(zip::CompressionMethod::ZSTD).unix_permissions(0o755);

            for (path, content) in entries {
                if let Some(content) = content {
                    zip.start_file(&path, options)?;
                    zip.write_all(&content)?;
                } else {
                    zip.add_directory(&path, options)?;
                }
            }

            zip.finish()?;
            Ok(())
        })
        .await??;

        File::new(None, out_path).await
    }
}

// Helper function to copy directories recursively
async fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<()> {
    fs::create_dir_all(&dst).await?;
    let mut entries = fs::read_dir(src).await?;

    while let Some(entry) = entries.next_entry().await? {
        let ty = entry.file_type().await?;
        let target = dst.as_ref().join(entry.file_name());

        if ty.is_dir() {
            copy_dir_all(entry.path(), target).await?;
        } else if ty.is_file() {
            fs::copy(entry.path(), target).await?;
        }
    }

    Ok(())
}
