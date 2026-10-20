use super::error::Result;
use super::service::GatewayService;

#[async_trait::async_trait]
/// Reconciles service definitions with an external API gateway.
///
/// Implementations should treat registration as idempotent so applications can
/// declare their desired routes during every startup.
pub trait ApiGatewayClient {
    /// Creates or updates a service and its routes and plugins.
    async fn register(&self, service: GatewayService) -> Result<()>;
    /// Loads a service by its gateway name.
    async fn get_service(&self, service_name: &str) -> Result<Option<GatewayService>>;
    /// Applies route configuration to an existing or newly registered service.
    async fn route_to(&self, service: GatewayService) -> Result<()>;
    /// Removes a service and its gateway-owned routes.
    async fn delete_service(&self, service_name: &str) -> Result<()>;
}
