pub use tokio_postgres::NoTls;

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_native_tls::native_tls::{Error as TlsError, TlsConnector};
use tokio_native_tls::TlsConnector as TokioTlsConnector;
use tokio_postgres::tls::{MakeTlsConnect, TlsConnect, TlsStream};
use tokio_postgres::Socket;

use crate::errors::PoolError;

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
pub struct NativeTlsConnector {
    inner: TokioTlsConnector,
}

impl NativeTlsConnector {
    #[must_use]
    pub fn new(connector: TlsConnector) -> Self {
        Self {
            inner: TokioTlsConnector::from(connector),
        }
    }

    #[must_use]
    pub fn inner(&self) -> &TokioTlsConnector {
        &self.inner
    }
}

impl MakeTlsConnect<Socket> for NativeTlsConnector {
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

/// Enum to configure the TLS connection
#[derive(Clone)]
pub enum Tls {
    Prefer,
    Require,
    VerifyCa,
    VerifyIdentity,
}

impl Tls {
    /// Configures TLS connection based on the selected TLS mode
    ///
    /// Creates a TLS connector with appropriate security settings for the
    /// selected mode:
    ///
    /// - `Prefer` or `Require` accepts invalid certificates and hostnames
    /// - `VerifyCa` requires valid certificates but accepts invalid hostnames
    /// - `VerifyIdentity` requires valid certificates and hostnames
    ///
    /// # Examples
    ///
    /// ```
    /// use nanopool::tls::Tls;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let tls = Tls::configure(Tls::Prefer)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a `PoolError` if the TLS connection cannot be built
    pub fn configure(self) -> Result<NativeTlsConnector, PoolError> {
        let mut builder = TlsConnector::builder();

        match self {
            Tls::Prefer | Tls::Require => {
                builder.danger_accept_invalid_certs(true);
                builder.danger_accept_invalid_hostnames(true);
            }
            Tls::VerifyCa => {
                builder.danger_accept_invalid_hostnames(true);
            }
            Tls::VerifyIdentity => {}
        }

        let connector = builder.build()?;
        Ok(NativeTlsConnector::new(connector))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefer_tls() {
        assert!(Tls::configure(Tls::Prefer).is_ok());
    }

    #[test]
    fn test_require_tls() {
        assert!(Tls::configure(Tls::Require).is_ok());
    }

    #[test]
    fn test_verify_ca_tls() {
        assert!(Tls::configure(Tls::VerifyCa).is_ok());
    }

    #[test]
    fn test_verify_identity_tls() {
        assert!(Tls::configure(Tls::VerifyIdentity).is_ok());
    }
}
