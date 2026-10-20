use super::error::Result;
use super::service::GatewayService;

#[async_trait::async_trait]
pub trait ApiGatewayClient {
    async fn register(&self, service: GatewayService) -> Result<()>;
    async fn get_service(&self, service_name: &str) -> Result<Option<GatewayService>>;
    async fn route_to(&self, service: GatewayService) -> Result<()>;
    async fn delete_service(&self, service_name: &str) -> Result<()>;
}
