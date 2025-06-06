use crate::{
    errors::PoolError,
    tls::native::{self, TlsConnector},
};

/// Enum to configure the TLS connection
#[derive(Copy, Clone)]
pub enum TlsMode {
    Prefer,
    Require,
    VerifyCa,
    VerifyIdentity,
}

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
/// use nanopool::tls;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let tls = tls::configure(tls::TlsMode::Prefer)?;
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns a `PoolError` if the TLS connection cannot be built
pub fn configure(mode: TlsMode) -> Result<TlsConnector, PoolError> {
    native::configure(mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefer_tls() {
        assert!(configure(TlsMode::Prefer).is_ok());
    }

    #[test]
    fn test_require_tls() {
        assert!(configure(TlsMode::Require).is_ok());
    }

    #[test]
    fn test_verify_ca_tls() {
        assert!(configure(TlsMode::VerifyCa).is_ok());
    }

    #[test]
    fn test_verify_identity_tls() {
        assert!(configure(TlsMode::VerifyIdentity).is_ok());
    }
}
