use std::future::Future;
use std::pin::Pin;

use tokio::io::BufReader;
use tokio_native_tls::{
    native_tls::{Error as TlsError, TlsConnector as NativeTlsConnector},
    TlsConnector as TokioTlsConnector,
};
use tokio_postgres::{
    tls::{ChannelBinding, MakeTlsConnect, TlsConnect, TlsStream},
    Socket,
};

use crate::errors::PoolError;
use crate::tls::{stream::GenericTlsStream, TlsMode};

pub type NativeTlsStream = GenericTlsStream<tokio_native_tls::TlsStream<BufReader<Socket>>>;

impl TlsStream for NativeTlsStream {
    fn channel_binding(&self) -> ChannelBinding {
        match self.inner().get_ref().tls_server_end_point().ok().flatten() {
            Some(buf) => ChannelBinding::tls_server_end_point(buf),
            None => ChannelBinding::none(),
        }
    }
}

#[derive(Clone)]
pub struct TlsConnector {
    inner: TokioTlsConnector,
}

impl TlsConnector {
    pub fn new(connector: NativeTlsConnector) -> Self {
        Self {
            inner: TokioTlsConnector::from(connector),
        }
    }

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
            let buffered = BufReader::new(stream);
            let tls_stream = self.connector.connect(&self.domain, buffered).await?;
            Ok(NativeTlsStream::new(tls_stream))
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
