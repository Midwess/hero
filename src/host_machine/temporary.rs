use std::fs::{create_dir_all, remove_dir_all};
use std::path::{Path, PathBuf};

use tokio::fs::{remove_file, File};
use tokio::spawn;

use crate::services::base::Resolve;

/**
 * A temp file/directory will could be created at path
 * and deleted after the instance dropped
 */
pub enum Temp {
    Directory(PathBuf),
    File(PathBuf, Option<File>)
}

impl AsRef<Path> for Temp {
    fn as_ref(&self) -> &Path {
        match self {
            Temp::Directory(path) => path.as_path(),
            Temp::File(path, _) => path.as_path()
        }
    }
}

impl AsMut<File> for Temp {
    fn as_mut(&mut self) -> &mut File {
        match self {
            Temp::File(_, file) => file.as_mut().unwrap(),
            _ => panic!("A directory can not be deref as a file")
        }
    }
}

impl Temp {
    pub async fn dir(path: PathBuf) -> Resolve<Self> {
        if path.is_dir() {
            return Ok(Self::Directory(path))
        }

        create_dir_all(path.as_path())?;

        return Ok(Self::Directory(path))
    }

    pub async fn file(path: PathBuf) -> Resolve<Self> {
        if path.parent().is_some() {
            create_dir_all(path.parent().unwrap())?;
        }

        if path.is_file() {
            let file = File::open(path.as_path()).await?;
            return Ok(Self::File(path, Some(file)))
        }

        let file = File::create_new(path.as_path()).await?;
        Ok(Self::File(path, Some(file)))
    }

    pub fn path(&self) -> PathBuf {
        match self {
            Temp::Directory(path) => path.clone(),
            Temp::File(path, _) => path.clone()
        }
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        match self {
            Temp::Directory(path) => {
                let path = path.clone();
                spawn(async move {
                    let _ = remove_dir_all(path.as_path());
                });
            }
            Temp::File(path, Some(_)) => {
                let path = path.clone();
                spawn(async move {
                    let _ = remove_file(path.as_path()).await;
                });
            }
            _ => {}
        };
    }
}
