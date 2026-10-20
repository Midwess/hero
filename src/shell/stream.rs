//! A line stream that polls a child's stdout before stderr.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncBufReadExt, BufReader, Lines};
use tokio::process::{Child, ChildStderr, ChildStdout};
use tokio_stream::Stream;

/// Merges piped stdout and stderr into a stream of text lines.
///
/// Constructing the stream requires both handles to be piped. I/O errors end
/// the stream because the item type intentionally contains only successful
/// lines. Ordering between stdout and stderr follows readiness, with stdout
/// checked first when both are ready.
pub struct StdStream {
    stdout: Lines<BufReader<ChildStdout>>,
    stderr: Lines<BufReader<ChildStderr>>,
}

impl TryFrom<Child> for StdStream {
    type Error = std::io::Error;

    fn try_from(mut value: Child) -> Result<Self, Self::Error> {
        let stdout = value
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "child stdout is not piped"))?;
        let stderr = value
            .stderr
            .take()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "child stderr is not piped"))?;
        Ok(Self {
            stdout: BufReader::new(stdout).lines(),
            stderr: BufReader::new(stderr).lines(),
        })
    }
}

impl Stream for StdStream {
    type Item = String;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let up = self.get_mut();
        let mut stdout = Box::pin(up.stdout.next_line());
        let mut stderr = Box::pin(up.stderr.next_line());

        match stdout.as_mut().poll(cx) {
            Poll::Ready(Ok(result)) => return Poll::Ready(result.map(|it| it.to_string())),
            Poll::Ready(Err(_)) => return Poll::Ready(None),
            _ => {}
        };

        match stderr.as_mut().poll(cx) {
            Poll::Ready(Ok(result)) => Poll::Ready(result.map(|it| it.to_string())),
            Poll::Ready(Err(_)) => Poll::Ready(None),
            _ => Poll::Pending,
        }
    }
}
