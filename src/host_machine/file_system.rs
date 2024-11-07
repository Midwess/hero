use std::path::Path;

use tokio::fs;

use crate::services::base::Resolve;

pub struct FileSystem {}

impl FileSystem {
    pub async fn copy_directory(&self, src: &Path, dst: &Path) -> Resolve<()> {
        while let Some(entry) = fs::read_dir(src).await?.next_entry().await? {
            let entry = entry;
            let path = entry.path();
            let dest_path = dst.join(entry.file_name());

            if path.is_dir() {
                Box::pin(self.copy_directory(&path, &dest_path)).as_mut().await?;
            } else {
                fs::copy(&path, &dest_path).await?;
            }
        }

        Ok(())
    }

    pub fn get_extension(&self, path: &Path) -> Option<String> {
        path.extension().map(|it| it.to_str().unwrap().to_string())
    }

    pub fn get_file_name(&self, path: &Path) -> Option<String> {
        path.file_name().map(|it| it.to_str().unwrap().to_string())
    }
}
