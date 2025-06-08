use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};
use tokio_postgres::{
    tls::{MakeTlsConnect, TlsConnect},
    Client as PgClient, Socket,
};

pub use tokio_postgres::Config;

use crate::errors::PoolError;

/// Messages that can be sent to the pool
pub enum PoolMessage {
    /// Request a client from the pool
    GetClient { response: oneshot::Sender<PgClient> },
    /// Return a client to the pool
    ReturnClient { client: Option<PgClient> },
}

/// A wrapper around the client that returns itself to the pool when dropped
pub struct Client {
    client: Option<PgClient>,
    pool: mpsc::Sender<PoolMessage>,
}

impl std::ops::Deref for Client {
    type Target = PgClient;

    fn deref(&self) -> &Self::Target {
        self.client.as_ref().unwrap()
    }
}

impl std::ops::DerefMut for Client {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.client.as_mut().unwrap()
    }
}

/// Return the client to the pool asynchronously
impl Drop for Client {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            let _ = self.pool.try_send(PoolMessage::ReturnClient {
                client: Some(client),
            });
        }
    }
}

/// A callback function that is invoked when a connection error occurs
type PoolFn = Box<dyn Fn(Result<(), PoolError>) + Send + Sync + 'static>;

/// A connection pool for PostgreSQL clients
#[derive(Clone)]
pub struct Pool {
    sender: mpsc::Sender<PoolMessage>,
}

impl Pool {
    /// Create a new connection pool from a connection string
    ///
    /// This method creates a new PostgreSQL connection pool with the specified
    /// TLS configuration and number of connections. It parses the provided
    /// connection string into a PostgreSQL `Config` and creates the connections
    /// immediately.
    ///
    /// # Examples
    ///
    /// ## Without TLS
    ///
    /// ```
    /// # use nanopool::pool::Pool;
    /// # use nanopool::tls::NoTls;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let pool = Pool::new(
    ///     "postgresql://postgres:password@localhost/mydb",
    ///     NoTls,
    ///     4
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## With TLS
    ///
    /// ```
    /// # use nanopool::pool::Pool;
    /// # use nanopool::tls;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let tls = tls::configure(tls::TlsMode::Prefer)?;
    /// let secure_pool = Pool::new(
    ///     "postgresql://postgres:password@localhost/mydb",
    ///     tls,
    ///     4
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a `PoolError` if the connection string cannot be parsed or the
    /// connections to the database cannot be established.
    ///
    /// # Notes
    ///
    /// If you need more control over the configuration, use `from_config` or
    /// `from_config_with_callback` instead.
    pub async fn new<T>(conn: impl Into<String>, tls: T, size: usize) -> Result<Self, PoolError>
    where
        T: MakeTlsConnect<Socket> + Clone + Send + 'static,
        <T as MakeTlsConnect<Socket>>::Stream: Send + 'static,
        <T as MakeTlsConnect<Socket>>::TlsConnect: Send,
        <<T as MakeTlsConnect<Socket>>::TlsConnect as TlsConnect<Socket>>::Future: Send,
    {
        let config = conn.into().parse::<Config>()?;
        Self::from_config(config, tls, size).await
    }

    /// Create a new connection pool from a configuration
    ///
    /// This method creates a new PostgreSQL connection pool with the specified
    /// TLS configuration and number of connections. It creates the connections
    /// immediately.
    ///
    /// # Examples
    ///
    /// ## Without TLS
    ///
    /// ```
    /// # use nanopool::pool::{Config, Pool};
    /// # use nanopool::tls::NoTls;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut config = Config::new();
    /// let config = config
    ///   .user("postgres")
    ///   .password("password")
    ///   .host("localhost")
    ///   .dbname("mydb")
    ///   .clone();
    ///
    /// let pool = Pool::from_config(config, NoTls, 4).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## With TLS
    ///
    /// ```
    /// # use nanopool::pool::Config;
    /// # use nanopool::pool::Pool;
    /// # use nanopool::tls;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut config = Config::new();
    /// let config = config
    ///   .user("postgres")
    ///   .password("password")
    ///   .host("localhost")
    ///   .dbname("mydb")
    ///   .clone();
    ///
    /// let tls = tls::configure(tls::TlsMode::Prefer)?;
    /// let secure_pool = Pool::from_config(config, tls, 4).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a `PoolError` if the connection string cannot be parsed or the
    /// connections to the database cannot be established.
    ///
    /// # Notes
    ///
    /// If you need to handle connection errors after a connection is
    /// established, use `from_config_with_callback` instead.
    pub async fn from_config<T>(config: Config, tls: T, size: usize) -> Result<Self, PoolError>
    where
        T: MakeTlsConnect<Socket> + Clone + Send + 'static,
        <T as MakeTlsConnect<Socket>>::Stream: Send + 'static,
        <T as MakeTlsConnect<Socket>>::TlsConnect: Send,
        <<T as MakeTlsConnect<Socket>>::TlsConnect as TlsConnect<Socket>>::Future: Send,
    {
        let (sender, receiver) = mpsc::channel::<PoolMessage>(size);
        Self::manage_pool(receiver, config, tls, size, Arc::new(None)).await?;
        Ok(Self { sender })
    }

    /// Create a new connection pool from a configuration with a callback
    ///
    /// This method creates a new PostgreSQL connection pool with the specified
    /// TLS configuration and number of connections. It creates the connections
    /// immediately and invokes the callback function when a connection error
    /// occurs after a connection is established.
    ///
    /// # Examples
    ///
    /// ## Without TLS
    ///
    /// ```
    /// # use nanopool::pool::{Config, Pool};
    /// # use nanopool::tls::NoTls;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut config = Config::new();
    /// let config = config
    ///   .user("postgres")
    ///   .password("password")
    ///   .host("localhost")
    ///   .dbname("mydb")
    ///   .clone();
    ///
    /// let pool = Pool::from_config_with_callback(
    ///   config,
    ///   NoTls,
    ///   4,
    ///   Box::new(|result| {
    ///     if let Err(err) = result {
    ///       eprintln!("Error: {}", err);
    ///     }
    ///   })
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## With TLS
    ///
    /// ```
    /// # use nanopool::pool::{Config, Pool};
    /// # use nanopool::tls;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut config = Config::new();
    /// let config = config
    ///   .user("postgres")
    ///   .password("password")
    ///   .host("localhost")
    ///   .dbname("mydb")
    ///   .clone();
    ///
    /// let tls = tls::configure(tls::TlsMode::Prefer)?;
    /// let secure_pool = Pool::from_config_with_callback(
    ///   config,
    ///   tls,
    ///   4,
    ///   Box::new(|result| {
    ///     if let Err(err) = result {
    ///       eprintln!("Error: {}", err);
    ///     }
    ///   })
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a `PoolError` if the connection string cannot be parsed or the
    /// connections to the database cannot be established.
    ///
    /// # Notes
    ///
    /// The callback function is invoked when a connection error occurs after
    /// the connection is established. The callback function is passed a
    /// `Result` that is `Ok(())` if the connection is successful and `Err` if
    /// an error occurs.
    pub async fn from_config_with_callback<T>(
        config: Config,
        tls: T,
        size: usize,
        callback: PoolFn,
    ) -> Result<Self, PoolError>
    where
        T: MakeTlsConnect<Socket> + Clone + Send + 'static,
        <T as MakeTlsConnect<Socket>>::Stream: Send + 'static,
        <T as MakeTlsConnect<Socket>>::TlsConnect: Send,
        <<T as MakeTlsConnect<Socket>>::TlsConnect as TlsConnect<Socket>>::Future: Send,
    {
        let (sender, receiver) = mpsc::channel::<PoolMessage>(size);
        Self::manage_pool(receiver, config, tls, size, Arc::new(Some(callback))).await?;
        Ok(Self { sender })
    }

    /// The background task that manages the pool
    async fn manage_pool<T>(
        mut receiver: mpsc::Receiver<PoolMessage>,
        config: Config,
        tls: T,
        size: usize,
        callback: Arc<Option<PoolFn>>,
    ) -> Result<(), PoolError>
    where
        T: MakeTlsConnect<Socket> + Clone,
        <T as MakeTlsConnect<Socket>>::Stream: Send + 'static,
    {
        let mut clients = Vec::with_capacity(size);
        let mut waiting = Vec::with_capacity(size);
        for _ in 0..size {
            let client = Self::connect(&config, tls.clone(), callback.clone()).await?;
            clients.push(client);
        }
        tokio::spawn(async move {
            while let Some(command) = receiver.recv().await {
                match command {
                    PoolMessage::GetClient { response } => {
                        if let Some(client) = clients.pop() {
                            let _ = response.send(client);
                        } else {
                            waiting.push(response);
                        }
                    }
                    PoolMessage::ReturnClient { client } => {
                        let client = client.unwrap(); // client is always Some
                        if let Some(waiter) = waiting.pop() {
                            let _ = waiter.send(client);
                        } else {
                            clients.push(client);
                        }
                    }
                }
            }
        });
        Ok(())
    }

    /// Create a new connection
    async fn connect<T>(
        config: &Config,
        tls: T,
        handler: Arc<Option<PoolFn>>,
    ) -> Result<PgClient, PoolError>
    where
        T: MakeTlsConnect<Socket>,
        <T as MakeTlsConnect<Socket>>::Stream: Send + 'static,
    {
        let (client, connection) = config.connect(tls).await?;
        tokio::spawn(async move {
            let result = connection.await.map_err(PoolError::from);
            // callback is only invoked if an error occurs _after_ the connection is established
            if let Some(handler) = handler.as_ref() {
                handler(result);
            }
        });
        Ok(client)
    }

    /// Acquire a client from the pool
    ///
    /// This method obtains a connection from the pool or waits until one
    /// becomes available. The returned `Client` automatically returns the
    /// underlying PostgreSQL connection to the pool when dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # use nanopool::pool::Pool;
    /// # use nanopool::tls::NoTls;
    /// # let pool = Pool::new("postgresql://postgres:postgres@localhost/postgres", NoTls, 4).await?;
    /// // Acquire a client from the pool
    /// let client = pool.client().await?;
    ///
    /// // Use the client for database operations
    /// let row = client.query_one("SELECT 1 + 2", &[]).await?;
    /// let sum: i32 = row.get(0);
    ///
    /// // The client will be automatically returned to the pool when it goes out of scope
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a `PoolError` if the client was closed or if there was an error
    /// communicating with the pool.
    ///
    /// # Notes
    ///
    /// The returned `Client` implements `Drop` to automatically return the
    /// connection to the pool, so there's no need to explicitly release it.
    pub async fn client(&self) -> Result<Client, PoolError> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(PoolMessage::GetClient { response: sender })
            .await?;
        let client = receiver.await?;
        Ok(Client {
            client: Some(client),
            pool: self.sender.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use tokio::task::JoinSet;

    use super::*;

    use crate::tls::{self, NoTls};

    const CONNECTION_STRING: &str = "postgresql://postgres:postgres@localhost/postgres";
    const POOL_SIZE: usize = 4;

    #[ignore = "requires a database connection"]
    #[tokio::test]
    async fn test_pool_notls() {
        let pool = Pool::new(CONNECTION_STRING, NoTls, POOL_SIZE)
            .await
            .unwrap();
        let client = pool.client().await.unwrap();
        let row = client.query_one("SELECT 1 + 2", &[]).await.unwrap();
        let sum: i32 = row.get(0);
        assert_eq!(sum, 3);
    }

    #[ignore = "requires a database connection"]
    #[tokio::test]
    async fn test_pool_tls() {
        let tls = tls::configure(tls::TlsMode::Prefer).unwrap();
        let pool = Pool::new(CONNECTION_STRING, tls, POOL_SIZE).await.unwrap();
        let client = pool.client().await.unwrap();
        let row = client.query_one("SELECT 1 + 2", &[]).await.unwrap();
        let sum: i32 = row.get(0);
        assert_eq!(sum, 3);
    }

    #[tokio::test]
    #[should_panic(expected = "both host and hostaddr are missing")]
    async fn test_pool_missing_host() {
        let _pool = Pool::new("", NoTls, POOL_SIZE).await.unwrap();
    }

    #[tokio::test]
    #[should_panic(expected = "Connection refused")]
    async fn test_pool_connection_refused() {
        let config = Config::new().host("localhost").port(65535).clone();
        let _pool = Pool::from_config(config, NoTls, POOL_SIZE).await.unwrap();
    }

    #[ignore = "requires a database connection"]
    #[tokio::test]
    async fn test_pool_client_query_error() {
        let pool = Pool::new(CONNECTION_STRING, NoTls, POOL_SIZE)
            .await
            .unwrap();
        let client = pool.client().await.unwrap();
        let _ = client
            .execute("SELECT pg_terminate_backend(pg_backend_pid())", &[])
            .await;
        assert!(client.query_one("SELECT 1 + 2", &[]).await.is_err());
    }

    #[ignore = "requires a database connection"]
    #[tokio::test]
    async fn test_pool_from_config() {
        let mut config = Config::new();
        let config = config
            .user("postgres")
            .password("postgres")
            .host("localhost")
            .dbname("postgres")
            .clone();
        let pool = Pool::from_config(config, NoTls, POOL_SIZE).await.unwrap();
        let client = pool.client().await.unwrap();
        let row = client.query_one("SELECT 1 + 2", &[]).await.unwrap();
        let sum: i32 = row.get(0);
        assert_eq!(sum, 3);
    }

    #[ignore = "requires a database connection"]
    #[tokio::test]
    async fn test_pool_from_config_with_callback() {
        let mut config = Config::new();
        let config = config
            .user("postgres")
            .password("postgres")
            .host("localhost")
            .dbname("postgres")
            .clone();
        let pool = Pool::from_config_with_callback(
            config,
            NoTls,
            POOL_SIZE,
            Box::new(|result| {
                assert!(result.is_err());
            }),
        )
        .await
        .unwrap();
        let client = pool.client().await.unwrap();
        let _ = client
            .execute("SELECT pg_terminate_backend(pg_backend_pid())", &[])
            .await;
        assert!(client.query_one("SELECT 1 + 2", &[]).await.is_err());
    }

    #[ignore = "requires a database connection"]
    #[tokio::test]
    async fn test_pool_mut_client() {
        let pool = Pool::new(CONNECTION_STRING, NoTls, POOL_SIZE)
            .await
            .unwrap();
        let mut client = pool.client().await.unwrap();
        let transaction = client.transaction().await.unwrap();
        let row = transaction.query_one("SELECT 1 + 2", &[]).await.unwrap();
        transaction.commit().await.unwrap();
        let sum: i32 = row.get(0);
        assert_eq!(sum, 3);
    }

    #[ignore = "requires a database connection"]
    #[tokio::test]
    async fn test_pool_stress() {
        let pool = Pool::new(CONNECTION_STRING, NoTls, POOL_SIZE)
            .await
            .unwrap();
        let mut join = JoinSet::new();
        // spawn tasks to simulate pool load
        for _i in 0..128 {
            let clone = pool.clone();
            join.spawn(async move {
                let client = clone.client().await.unwrap();
                let row = client
                    .query_one("SELECT random() FROM pg_sleep(0.1)", &[])
                    .await
                    .unwrap();
                let id: f64 = row.get(0);
                id
            });
        }
        let results = join.join_all().await;
        assert!(!results.is_empty());
    }
}
