use crate::pool::PoolMessage;

/// Error handling for the pool
#[derive(Debug)]
pub enum PoolError {
    /// A database operation failed
    DatabaseError(tokio_postgres::Error),
    /// A message could not be received
    RecvError(tokio::sync::oneshot::error::RecvError),
    /// A message could not be sent
    SendError(tokio::sync::mpsc::error::SendError<PoolMessage>),
    /// A TLS error occurred
    TlsError(native_tls::Error),
}

/// Convert `PoolError` to a string
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

/// Convert `tokio_postgres::Error` to `PoolError`
impl From<tokio_postgres::Error> for PoolError {
    fn from(kind: tokio_postgres::Error) -> Self {
        PoolError::DatabaseError(kind)
    }
}

/// Convert `tokio::sync::oneshot::error::RecvError` to `PoolError`
impl From<tokio::sync::oneshot::error::RecvError> for PoolError {
    fn from(kind: tokio::sync::oneshot::error::RecvError) -> Self {
        PoolError::RecvError(kind)
    }
}

/// Convert `tokio::sync::mpsc::error::SendError<PoolMessage>` to `PoolError`
impl From<tokio::sync::mpsc::error::SendError<PoolMessage>> for PoolError {
    fn from(kind: tokio::sync::mpsc::error::SendError<PoolMessage>) -> Self {
        PoolError::SendError(kind)
    }
}

/// Convert `native_tls::Error` to `PoolError`
impl From<native_tls::Error> for PoolError {
    fn from(kind: native_tls::Error) -> Self {
        PoolError::TlsError(kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pool_error_database() {
        let pg_error = tokio_postgres::connect("test.invalid", tokio_postgres::NoTls)
            .await
            .err()
            .unwrap();
        let error = PoolError::from(pg_error);
        assert_eq!(
            format!("{error}"),
            "invalid connection string: unexpected EOF"
        );
    }

    #[tokio::test]
    async fn test_pool_error_recv() {
        let (sender, receiver) = tokio::sync::oneshot::channel::<i32>();
        drop(sender);
        let recv_error = receiver.await.unwrap_err();
        let error = PoolError::from(recv_error);
        assert_eq!(format!("{error}"), "channel closed");
    }

    #[tokio::test]
    async fn test_pool_error_send() {
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        drop(receiver);
        let send_error = sender
            .send(PoolMessage::ReturnClient { client: None })
            .await
            .unwrap_err();
        let error = PoolError::from(send_error);
        assert_eq!(format!("{error}"), "channel closed");
    }

    #[test]
    fn test_pool_error_tls() {
        let tls_error = native_tls::Identity::from_pkcs8(&[0xff], &[0xff])
            .err()
            .unwrap();
        let error = PoolError::from(tls_error);
        assert_eq!(format!("{error}"), "expected PKCS#8 PEM");
    }
}
