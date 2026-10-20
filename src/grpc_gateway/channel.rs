use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tonic::transport::{Channel, Endpoint};

use crate::config::CONFIGS;

#[derive(Clone)]
/// Lazily connected tonic channel shared by all clones.
///
/// The endpoint is captured from [`CONFIGS`] during
/// construction. A successful channel is cached; connection failures leave the
/// cache empty so the next call can retry.
pub struct GrpcGatewayChannel {
    channel: Arc<Mutex<Option<Channel>>>,
    endpoint: Endpoint,
}

impl GrpcGatewayChannel {
    /// Parses the configured Kong gateway URL into a tonic endpoint.
    pub fn new() -> Result<Self, tonic::transport::Error> {
        Ok(Self {
            channel: Arc::new(Mutex::new(None)),
            endpoint: CONFIGS.kong.url.parse()?,
        })
    }

    /// Connects with Hero's five-second default timeout or reuses the cache.
    pub async fn connect(&self) -> Result<Channel, tonic::transport::Error> {
        self.connect_with_timeout(Duration::from_secs(5)).await
    }

    /// Reports whether this wrapper has cached a successfully created channel.
    ///
    /// This does not probe transport health.
    pub async fn is_connected(&self) -> bool {
        self.channel.lock().await.is_some()
    }

    /// Connects with a caller-selected timeout or reuses the cached channel.
    pub async fn connect_with_timeout(&self, timeout: Duration) -> Result<Channel, tonic::transport::Error> {
        if let Some(channel) = self.channel.lock().await.clone() {
            return Ok(channel);
        }

        let channel = self.endpoint.clone().connect_timeout(timeout).connect().await?;
        self.channel.lock().await.replace(channel.clone());
        Ok(channel)
    }
}
