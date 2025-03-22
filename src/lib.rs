/// Connection pool errors
pub mod errors;
/// PostgreSQL connection pool
pub mod pool;
/// TLS connections
pub mod tls;

/// Re-export the `uuid` type
#[cfg(feature = "uuid")]
pub mod uuid;
