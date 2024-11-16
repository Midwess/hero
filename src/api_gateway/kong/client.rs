use core_services::services::base::Resolve;
use core_services::services::errors::Errors;
use reqwest::StatusCode;

use crate::api_gateway::client::ApiGatewayClient;
use crate::api_gateway::service::GatewayService;

pub struct KongGatewayAdminClient {
    pub url: String
}

#[async_trait::async_trait]
impl ApiGatewayClient for KongGatewayAdminClient {
    async fn route_to(&self, service: GatewayService) -> Resolve<()> {
        let kong_url = self.url.clone();
        let service_name = service.name.as_str();
        let client = reqwest::Client::new();
        for route in service.routes {
            let resp = client
                .post(format!("{kong_url}/services/{service_name}/routes"))
                .json(&route)
                .send()
                .await
                .map_err(|it| {
                    Errors::UnnableToRegisterApiGateway(format!("Unnable to register route {}", it.to_string()))
                })?;

            let resp = match resp.status() {
                StatusCode::CONFLICT => {
                    let resp = client
                        .patch(format!("{kong_url}/services/{service_name}/routes/{}", &route.name))
                        .json(&route)
                        .send()
                        .await
                        .map_err(|it| {
                            Errors::UnnableToRegisterApiGateway(format!("Unnable to register route {}", it.to_string()))
                        })?;

                    resp
                }
                _ => resp
            };

            if !resp.status().is_success() {
                return Err(Errors::UnnableToRegisterApiGateway(format!(
                    "{} {:?}",
                    resp.status(),
                    resp.text().await.unwrap()
                )))
            }
        }

        Ok(())
    }

    async fn register(&self, service: GatewayService) -> Resolve<()> {
        let kong_url = self.url.as_str();
        let client = reqwest::Client::new();

        let resp = client.post(format!("{kong_url}/services")).json(&service).send().await.map_err(|it| {
            Errors::UnnableToRegisterApiGateway(format!(
                "Unnable to register service, failed to create {}",
                it.to_string()
            ))
        })?;

        let resp = match resp.status() {
            StatusCode::CONFLICT => {
                let resp = client
                    .patch(format!("{kong_url}/services/{}", &service.name))
                    .json(&service)
                    .send()
                    .await
                    .map_err(|it| {
                    Errors::UnnableToRegisterApiGateway(format!(
                        "Unnable to register service, failed to update {}",
                        it.to_string()
                    ))
                })?;
                resp
            }
            _ => resp
        };

        if !resp.status().is_success() {
            return Err(Errors::UnnableToRegisterApiGateway(resp.status().to_string()))
        } else {
            if !resp.status().is_success() {
                return Err(Errors::UnnableToRegisterApiGateway(format!(
                    "Failed to update service {} {:?}",
                    resp.status().to_string(),
                    resp.text().await
                )))
            }
        }

        self.route_to(service).await?;
        Ok(())
    }

    async fn get_service(&self, service_name: &str) -> Resolve<Option<GatewayService>> {
        let kong_url = self.url.as_str();
        let resp = reqwest::get(format!("{kong_url}/services/{service_name}"))
            .await
            .map_err(|_it| Errors::GatewayOperationFailed("Failed to retrieving service info".to_owned()))?;

        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(None)
        }

        let service: GatewayService = resp.json().await.map_err(|it| Errors::GatewayOperationFailed(it.to_string()))?;

        return Ok(Some(service))
    }
}
