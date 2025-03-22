use crate::pool::PoolMessage;

#[derive(Debug)]
pub enum PoolError {
    DatabaseError(tokio_postgres::Error),
    RecvError(tokio::sync::oneshot::error::RecvError),
    SendError(tokio::sync::mpsc::error::SendError<PoolMessage>),
    TlsError(native_tls::Error),
}

impl std::fmt::Display for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PoolError::DatabaseError(err) => std::fmt::Display::fmt(err, f),
            PoolError::TlsError(err) => std::fmt::Display::fmt(err, f),
            PoolError::RecvError(err) => std::fmt::Display::fmt(err, f),
            PoolError::SendError(err) => std::fmt::Display::fmt(err, f),
        }
    }
}

impl From<tokio_postgres::Error> for PoolError {
    fn from(kind: tokio_postgres::Error) -> Self {
        PoolError::DatabaseError(kind)
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for PoolError {
    fn from(kind: tokio::sync::oneshot::error::RecvError) -> Self {
        PoolError::RecvError(kind)
    }
}

impl From<tokio::sync::mpsc::error::SendError<PoolMessage>> for PoolError {
    fn from(kind: tokio::sync::mpsc::error::SendError<PoolMessage>) -> Self {
        PoolError::SendError(kind)
    }
}

impl From<native_tls::Error> for PoolError {
    fn from(kind: native_tls::Error) -> Self {
        PoolError::TlsError(kind)
    }
}
