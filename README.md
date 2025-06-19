# nanopool

A fast and tiny asynchronous database pool for PostgreSQL.

The pool is built on top of the Tokio ecosystem to minimize non-Tokio dependencies and maximize performance. It also aims to be easy to use, with a simple API and minimal configuration. The pool has built-in TLS support, and optional features like `uuid` can be enabled for additional PostgreSQL functionality. It purposely does not support other databases to keep the codebase small and focused.

The pool is backed by a Tokio `mpsc` channel, which is optimized for fast concurrent access. Database operations are executed using `tokio-postgres`, which is a high-performance asynchronous PostgreSQL driver. Secure connections to PostgreSQL are supported through the `tokio-native-tls` crate. The core pool implementation is <200 lines of code, making it lightweight and easy to understand.

## Usage

1. Add `nanopool` to your `Cargo.toml`:

```toml
[dependencies]
nanopool = "1.0"
```

2. Create and use a connection pool:

```rust
use nanopool::{Pool, tls};

// create the pool with TLS support
let tls = tls::configure(tls::TlsMode::Require).unwrap();
let pool_size = 4;
let pool = Pool::new("postgresql://postgres:postgres@localhost/postgres", tls, pool_size).await.unwrap();

// or without TLS support
// let pool = Pool::new("postgresql://postgres:postgres@localhost/postgres", tls::NoTls, pool_size).await.unwrap();

// returns a `tokio_postgres::Client` instance
let client = pool.client().await.unwrap();

// execute a query
let row = client.query_one("SELECT 1 + 2", &[]).await.unwrap();
let sum: i32 = row.get(0);

// the client is automatically returned to the pool when dropped
```

## Optional features

- `uuid`: Enables support for UUIDs so they can be used in and returned from queries. This feature depends on the `uuid` crate. These can then be used from your code with `nanopool::uuid::Uuid`.

## Development

### Building

To build the library:

```sh
cargo build
```

### Testing

To run unit tests:

```sh
cargo test
```

To also run integration tests against a PostgreSQL database:

```sh
docker compose -f tests/docker-compose.test.yml up -d
cargo test -- --include-ignored
```
