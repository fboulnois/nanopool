pub use native_tls::{Certificate, TlsConnector};
pub use postgres_native_tls::MakeTlsConnector;
pub use tokio_postgres::NoTls;

use crate::errors::PoolError;

#[derive(Clone)]
pub enum Tls {
    Prefer,
    Require,
    VerifyCa,
    VerifyIdentity,
}

impl Tls {
    pub fn configure(self) -> Result<MakeTlsConnector, PoolError> {
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
        Ok(MakeTlsConnector::new(connector))
    }
}
