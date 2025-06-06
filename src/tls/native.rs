use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_native_tls::native_tls::{Error as TlsError, TlsConnector as NativeTlsConnector};
use tokio_native_tls::TlsConnector as TokioTlsConnector;
use tokio_postgres::tls::{MakeTlsConnect, TlsConnect, TlsStream};
use tokio_postgres::Socket;

use crate::errors::PoolError;
use crate::tls::TlsMode;

pub struct NativeTlsStream(tokio_native_tls::TlsStream<Socket>);

impl TlsStream for NativeTlsStream {
    fn channel_binding(&self) -> tokio_postgres::tls::ChannelBinding {
        tokio_postgres::tls::ChannelBinding::none()
    }
}

impl AsyncRead for NativeTlsStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl AsyncWrite for NativeTlsStream {
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

#[derive(Clone)]
pub struct TlsConnector {
    inner: TokioTlsConnector,
}

impl TlsConnector {
    #[must_use]
    pub fn new(connector: NativeTlsConnector) -> Self {
        Self {
            inner: TokioTlsConnector::from(connector),
        }
    }

    #[must_use]
    pub fn inner(&self) -> &TokioTlsConnector {
        &self.inner
    }
}

impl MakeTlsConnect<Socket> for TlsConnector {
    type Stream = NativeTlsStream;
    type TlsConnect = NativeTlsConnect;
    type Error = TlsError;

    fn make_tls_connect(&mut self, domain: &str) -> Result<Self::TlsConnect, Self::Error> {
        Ok(NativeTlsConnect {
            connector: self.inner.clone(),
            domain: domain.to_string(),
        })
    }
}

pub struct NativeTlsConnect {
    connector: TokioTlsConnector,
    domain: String,
}

impl TlsConnect<Socket> for NativeTlsConnect {
    type Stream = NativeTlsStream;
    type Error = TlsError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Stream, Self::Error>> + Send>>;

    fn connect(self, stream: Socket) -> Self::Future {
        Box::pin(async move {
            let tls_stream = self.connector.connect(&self.domain, stream).await?;
            Ok(NativeTlsStream(tls_stream))
        })
    }
}

pub fn configure(mode: TlsMode) -> Result<TlsConnector, PoolError> {
    let mut builder = NativeTlsConnector::builder();

    match mode {
        TlsMode::Prefer | TlsMode::Require => {
            builder.danger_accept_invalid_certs(true);
            builder.danger_accept_invalid_hostnames(true);
        }
        TlsMode::VerifyCa => {
            builder.danger_accept_invalid_hostnames(true);
        }
        TlsMode::VerifyIdentity => {}
    }

    let connector = builder.build()?;
    Ok(TlsConnector::new(connector))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_configure_native_tls() {
        assert!(configure(TlsMode::Prefer).is_ok());
    }
}
