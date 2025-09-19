use futures::channel::mpsc;
use futures::task::{Context, Poll};
use futures::io::{AsyncRead, AsyncWrite};
use n0_future::StreamExt;
use std::pin::Pin;
use std::task::Poll::{Ready, Pending};

pub struct DuplexWriter {
    sender: mpsc::UnboundedSender<u8>,
}

pub struct DuplexReader {
    receiver: mpsc::UnboundedReceiver<u8>,
}

pub fn duplex() -> (DuplexWriter, DuplexReader) {
    let (tx, rx) = mpsc::unbounded();
    (DuplexWriter { sender: tx }, DuplexReader { receiver: rx })
}

impl AsyncWrite for DuplexWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        let mut count = 0;

        while count < buf.len() {
            match self.sender.poll_ready(cx) {
                Poll::Ready(Ok(())) => {
                    match self.sender.unbounded_send(buf[count]) {
                        Ok(()) => {
                            count += 1;
                        }
                        Err(_) => {
                            return Ready(Err(std::io::Error::new(
                                std::io::ErrorKind::BrokenPipe,
                                "channel closed",
                            )));
                        }
                    }
                }
                Poll::Ready(Err(_)) => {
                    return Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "channel closed",
                    )));
                }
                Poll::Pending => {
                    if count == 0 {
                        return Pending;
                    }
                    break;
                }
            }
        }

        Ready(Ok(count))
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Ok(()).into()
    }

    fn poll_close(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Ok(()).into()
    }
}

impl AsyncRead for DuplexReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        if buf.is_empty() {
            return Ready(Ok(0));
        }

        let mut count = 0;

        while count < buf.len() {
            match Pin::new(&mut self.receiver).poll_next(cx) {
                Ready(Some(b)) => {
                    buf[count] = b;
                    count += 1;
                }
                Ready(None) => {
                    return Ready(Ok(count));
                }
                Pending => {
                    break;
                }
            }
        }

        Ready(Ok(count))
    }
}
