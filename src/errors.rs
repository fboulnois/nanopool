use crate::pool::PoolMessage;

/// Error handling for the pool
#[derive(Debug)]
pub enum PoolError {
    /// A database operation failed
    Database(tokio_postgres::Error),
    /// A message could not be received
    Recv(tokio::sync::oneshot::error::RecvError),
    /// A message could not be sent
    Send(tokio::sync::mpsc::error::SendError<PoolMessage>),
    /// A TLS error occurred
    Tls(native_tls::Error),
}

/// Convert `PoolError` to a string
impl std::fmt::Display for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PoolError::Database(err) => std::fmt::Display::fmt(err, f),
            PoolError::Tls(err) => std::fmt::Display::fmt(err, f),
            PoolError::Recv(err) => std::fmt::Display::fmt(err, f),
            PoolError::Send(err) => std::fmt::Display::fmt(err, f),
        }
    }
}

/// Implement `std::error::Error` for `PoolError`
impl std::error::Error for PoolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PoolError::Database(err) => Some(err),
            PoolError::Tls(err) => Some(err),
            PoolError::Recv(err) => Some(err),
            PoolError::Send(err) => Some(err),
        }
    }
}

/// Convert `tokio_postgres::Error` to `PoolError`
impl From<tokio_postgres::Error> for PoolError {
    fn from(kind: tokio_postgres::Error) -> Self {
        PoolError::Database(kind)
    }
}

/// Convert `tokio::sync::oneshot::error::RecvError` to `PoolError`
impl From<tokio::sync::oneshot::error::RecvError> for PoolError {
    fn from(kind: tokio::sync::oneshot::error::RecvError) -> Self {
        PoolError::Recv(kind)
    }
}

/// Convert `tokio::sync::mpsc::error::SendError<PoolMessage>` to `PoolError`
impl From<tokio::sync::mpsc::error::SendError<PoolMessage>> for PoolError {
    fn from(kind: tokio::sync::mpsc::error::SendError<PoolMessage>) -> Self {
        PoolError::Send(kind)
    }
}

/// Convert `native_tls::Error` to `PoolError`
impl From<native_tls::Error> for PoolError {
    fn from(kind: native_tls::Error) -> Self {
        PoolError::Tls(kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::error::Error;

    #[tokio::test]
    async fn test_pool_error_database() {
        let pg_error = tokio_postgres::connect("test.invalid", tokio_postgres::NoTls)
            .await
            .err()
            .unwrap();
        let error = PoolError::from(pg_error);
        let source = error.source().unwrap();
        let message = "invalid connection string: unexpected EOF";
        assert_eq!(format!("{error}"), message);
        assert_eq!(format!("{source}"), message);
    }

    #[tokio::test]
    async fn test_pool_error_recv() {
        let (sender, receiver) = tokio::sync::oneshot::channel::<i32>();
        drop(sender);
        let recv_error = receiver.await.unwrap_err();
        let error = PoolError::from(recv_error);
        let source = error.source().unwrap();
        let message = "channel closed";
        assert_eq!(format!("{error}"), message);
        assert_eq!(format!("{source}"), message);
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
        let source = error.source().unwrap();
        let message = "channel closed";
        assert_eq!(format!("{error}"), message);
        assert_eq!(format!("{source}"), message);
    }

    #[test]
    fn test_pool_error_tls() {
        let tls_error = native_tls::Identity::from_pkcs8(&[0xff], &[0xff])
            .err()
            .unwrap();
        let error = PoolError::from(tls_error);
        let source = error.source().unwrap();
        let message = "expected PKCS#8 PEM";
        assert_eq!(format!("{error}"), message);
        assert_eq!(format!("{source}"), message);
    }
}
