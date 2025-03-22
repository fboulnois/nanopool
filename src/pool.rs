use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};
use tokio_postgres::{
    tls::{MakeTlsConnect, TlsConnect},
    Client as PgClient, Socket,
};

pub use tokio_postgres::Config;

use crate::errors::PoolError;

pub enum PoolMessage {
    GetClient { response: oneshot::Sender<PgClient> },
    ReturnClient { client: Option<PgClient> },
}

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

impl Drop for Client {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            let pool = self.pool.clone();
            tokio::spawn(async move {
                let _ = pool
                    .send(PoolMessage::ReturnClient {
                        client: Some(client),
                    })
                    .await;
            });
        }
    }
}

type PoolFn = Box<dyn Fn(Result<(), PoolError>) + Send + Sync + 'static>;

#[derive(Clone)]
pub struct Pool {
    sender: mpsc::Sender<PoolMessage>,
}

impl Pool {
    pub fn new<T>(conn: impl Into<String>, tls: T, size: usize) -> Result<Self, PoolError>
    where
        T: MakeTlsConnect<Socket> + Clone + Send + 'static,
        <T as MakeTlsConnect<Socket>>::Stream: Send + 'static,
        <T as MakeTlsConnect<Socket>>::TlsConnect: Send,
        <<T as MakeTlsConnect<Socket>>::TlsConnect as TlsConnect<Socket>>::Future: Send,
    {
        let config = conn.into().parse::<Config>()?;
        Self::from_config(config, tls, size)
    }

    pub fn from_config<T>(config: Config, tls: T, size: usize) -> Result<Self, PoolError>
    where
        T: MakeTlsConnect<Socket> + Clone + Send + 'static,
        <T as MakeTlsConnect<Socket>>::Stream: Send + 'static,
        <T as MakeTlsConnect<Socket>>::TlsConnect: Send,
        <<T as MakeTlsConnect<Socket>>::TlsConnect as TlsConnect<Socket>>::Future: Send,
    {
        let (sender, receiver) = mpsc::channel::<PoolMessage>(size);
        tokio::spawn(Self::manage_pool(
            receiver,
            config,
            tls,
            size,
            Arc::new(None),
        ));
        Ok(Self { sender })
    }

    pub fn from_config_with_callback<T>(
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
        tokio::spawn(Self::manage_pool(
            receiver,
            config,
            tls,
            size,
            Arc::new(Some(callback)),
        ));
        Ok(Self { sender })
    }

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
        let mut waiting = Vec::new();
        for _ in 0..size {
            let client = Self::connect(&config, tls.clone(), callback.clone()).await?;
            clients.push(client);
        }
        while let Some(command) = receiver.recv().await {
            match command {
                PoolMessage::GetClient { response } => {
                    if let Some(client) = clients.pop() {
                        let _ = response.send(client);
                    } else {
                        waiting.push(response);
                    };
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
        Ok(())
    }

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

    use crate::tls::{NoTls, Tls};

    const CONNECTION_STRING: &'static str = "postgresql://postgres:postgres@localhost/postgres";
    const POOL_SIZE: usize = 4;

    #[ignore = "requires a database connection"]
    #[tokio::test]
    async fn test_pool_notls() {
        let pool = Pool::new(CONNECTION_STRING, NoTls, POOL_SIZE).unwrap();
        let client = pool.client().await.unwrap();
        let row = client.query_one("SELECT 1 + 2", &[]).await.unwrap();
        let sum: i32 = row.get(0);
        assert_eq!(sum, 3);
    }

    #[ignore = "requires a database connection"]
    #[tokio::test]
    async fn test_pool_tls() {
        let tls = Tls::configure(Tls::Prefer).unwrap();
        let pool = Pool::new(CONNECTION_STRING, tls, POOL_SIZE).unwrap();
        let client = pool.client().await.unwrap();
        let row = client.query_one("SELECT 1 + 2", &[]).await.unwrap();
        let sum: i32 = row.get(0);
        assert_eq!(sum, 3);
    }

    #[ignore = "requires a database connection"]
    #[tokio::test]
    async fn test_pool_error() {
        let pool = Pool::new(CONNECTION_STRING, NoTls, POOL_SIZE).unwrap();
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
        let pool = Pool::from_config(config, NoTls, POOL_SIZE).unwrap();
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
        let pool = Pool::new(CONNECTION_STRING, NoTls, POOL_SIZE).unwrap();
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
        let pool = Pool::new(CONNECTION_STRING, NoTls, POOL_SIZE).unwrap();
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
        assert!(results.len() > 0);
    }
}
