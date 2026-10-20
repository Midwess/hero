use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tonic::transport::{Channel, Endpoint};

use crate::config::CONFIGS;

// A singleton channel that open a connection to the gateway
// and because every services is connected to gateway, they all be shared the same channel
#[derive(Clone)]
pub struct GrpcGatewayChannel {
    channel: Arc<Mutex<Option<Channel>>>,
    endpoint: Endpoint,
}

impl GrpcGatewayChannel {
    pub fn new() -> Result<Self, tonic::transport::Error> {
        Ok(Self {
            channel: Arc::new(Mutex::new(None)),
            endpoint: CONFIGS.kong.url.parse()?,
        })
    }

    pub async fn connect(&self) -> Result<Channel, tonic::transport::Error> {
        self.connect_with_timeout(Duration::from_secs(5)).await
    }

    pub async fn is_connected(&self) -> bool {
        self.channel.lock().await.is_some()
    }

    pub async fn connect_with_timeout(&self, timeout: Duration) -> Result<Channel, tonic::transport::Error> {
        if let Some(channel) = self.channel.lock().await.clone() {
            return Ok(channel);
        }

        let channel = self.endpoint.clone().connect_timeout(timeout).connect().await?;
        self.channel.lock().await.replace(channel.clone());
        Ok(channel)
    }
}
