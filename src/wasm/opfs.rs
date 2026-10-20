//! Async-write adaptation for Origin Private File System handles.

use futures::io::AsyncWrite;
use std::io::Error;
use std::pin::Pin;
use std::task::{Context, Poll};
use web_sys::FileSystemSyncAccessHandle;

/// Sequential writer over a browser [`FileSystemSyncAccessHandle`].
///
/// This adapter is useful for libraries that expect `futures::io::AsyncWrite`,
/// including streaming archive writers. Closing flushes and closes the browser
/// handle. Browser exceptions are surfaced as [`std::io::Error`].
pub struct SyncAccessHandleWriter {
    handle: FileSystemSyncAccessHandle,
    position: u64,
}

impl SyncAccessHandleWriter {
    /// Starts writing at byte offset zero.
    pub fn new(handle: FileSystemSyncAccessHandle) -> Self {
        Self { handle, position: 0 }
    }

    /// Returns the next byte offset that will be written.
    pub fn position(&self) -> u64 {
        self.position
    }
}

impl AsyncWrite for SyncAccessHandleWriter {
    fn poll_write(mut self: Pin<&mut Self>, _cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, Error>> {
        let options = web_sys::FileSystemReadWriteOptions::new();
        options.set_at(self.position as f64);

        match self.handle.write_with_u8_array_and_options(buf, &options) {
            Ok(_) => {
                self.position += buf.len() as u64;
                Poll::Ready(Ok(buf.len()))
            }
            Err(e) => Poll::Ready(Err(Error::other(format!("{:?}", e)))),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        match self.handle.flush() {
            Ok(_) => Poll::Ready(Ok(())),
            Err(e) => Poll::Ready(Err(Error::other(format!("{:?}", e)))),
        }
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        let this = self.get_mut();
        match this.handle.flush() {
            Ok(_) => {
                this.handle.close();
                Poll::Ready(Ok(()))
            }
            Err(e) => Poll::Ready(Err(Error::other(format!("{:?}", e)))),
        }
    }
}
