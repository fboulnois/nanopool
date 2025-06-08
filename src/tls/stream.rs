use std::{
    pin::Pin,
    task::{Context, Poll},
};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// A generic TLS stream wrapper
pub struct GenericTlsStream<T>(T);

impl<T> GenericTlsStream<T> {
    pub fn new(stream: T) -> Self {
        Self(stream)
    }

    pub fn inner(&self) -> &T {
        &self.0
    }
}

/// Implementation of `AsyncRead` for `GenericTlsStream`
impl<T: AsyncRead + AsyncWrite + Unpin> AsyncRead for GenericTlsStream<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

/// Implementation of `AsyncWrite` for `GenericTlsStream`
impl<T: AsyncRead + AsyncWrite + Unpin> AsyncWrite for GenericTlsStream<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}
