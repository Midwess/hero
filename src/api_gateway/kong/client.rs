use core_services::services::base::Resolve;
use core_services::services::errors::Errors;
use log::info;
use reqwest::StatusCode;
use serde_json::json;

use crate::api_gateway::client::ApiGatewayClient;
use crate::api_gateway::service::GatewayService;

pub struct KongGatewayAdminClient {
    pub url: String
}

impl KongGatewayAdminClient {
    async fn handle_web_plugin(&self, route_name: String, enable_web: bool) -> Resolve<()> {
        let kong_url = self.url.clone();
        let client = reqwest::Client::new();

        if enable_web {
            // Add the grpc-web plugin to the route
            let resp = client
                .post(format!("{kong_url}/routes/{route_name}/plugins"))
                .json(&serde_json::json!({ "name": "grpc-web" }))
                .send()
                .await
                .map_err(|it| Errors::UnnableToRegisterApiGateway(format!("Unable to enable grpc-web plugin: {it}")))?;

            if !resp.status().is_success() && StatusCode::CONFLICT != resp.status() {
                return Err(Errors::UnnableToRegisterApiGateway(format!(
                    "Failed to enable grpc-web plugin: {} {:?}",
                    resp.status(),
                    resp.text().await.unwrap()
                )));
            }
        } else {
            // Find all plugins associated with the route
            let resp = client.get(format!("{kong_url}/routes/{route_name}/plugins")).send().await.map_err(|it| {
                Errors::UnnableToRegisterApiGateway(format!("Failed to retrieve plugins for route {route_name}: {it}"))
            })?;

            if !resp.status().is_success() {
                return Err(Errors::UnnableToRegisterApiGateway(format!(
                    "Failed to retrieve plugins: {} {:?}",
                    resp.status(),
                    resp.text().await.unwrap()
                )));
            }

            let plugins: serde_json::Value = resp
                .json()
                .await
                .map_err(|it| Errors::UnnableToRegisterApiGateway(format!("Failed to parse plugins response: {it}")))?;

            // Find and delete the grpc-web plugin
            if let Some(plugin_id) = plugins["data"].as_array().and_then(|plugins| {
                plugins.iter().find_map(|plugin| {
                    if plugin["name"] == "grpc-web" {
                        plugin["id"].as_str().map(|id| id.to_string())
                    } else {
                        None
                    }
                })
            }) {
                let _ = client.delete(format!("{kong_url}/plugins/{plugin_id}")).send().await.map_err(|it| {
                    Errors::UnnableToRegisterApiGateway(format!("Failed to delete grpc-web plugin: {it}"))
                })?;
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl ApiGatewayClient for KongGatewayAdminClient {
    async fn route_to(&self, service: GatewayService) -> Resolve<()> {
        let kong_url = self.url.clone();
        let service_name = service.name.as_str();
        let client = reqwest::Client::new();
        for route in service.routes {
            info!(target: "kong-admin", "Creating route {:?}", json!({"body": route}));
            let resp = client
                .post(format!("{kong_url}/services/{service_name}/routes"))
                .json(&route)
                .send()
                .await
                .map_err(|it| Errors::UnnableToRegisterApiGateway(format!("Unnable to register route {it}")))?;

            let resp = match resp.status() {
                StatusCode::CONFLICT => {
                    info!(target: "kong-admin", "Route already exist, change to update");

                    client
                        .patch(format!("{kong_url}/services/{service_name}/routes/{}", &route.name))
                        .json(&route)
                        .send()
                        .await
                        .map_err(|it| Errors::UnnableToRegisterApiGateway(format!("Unnable to register route {it}")))?
                }
                _ => resp
            };

            if !resp.status().is_success() {
                return Err(Errors::UnnableToRegisterApiGateway(format!(
                    "{} {:?}",
                    resp.status(),
                    resp.text().await.unwrap()
                )));
            }

            self.handle_web_plugin(route.name.clone(), route.enable_web).await?;
        }

        Ok(())
    }

    async fn register(&self, service: GatewayService) -> Resolve<()> {
        let kong_url = self.url.as_str();
        let client = reqwest::Client::new();

        info!(target: "kong-admin", "Creating service {}", json!({"body": service}));
        let resp = client.post(format!("{kong_url}/services")).json(&service).send().await.map_err(|it| {
            Errors::UnnableToRegisterApiGateway(format!("Unnable to register service, failed to create {it}"))
        })?;

        let resp = match resp.status() {
            StatusCode::CONFLICT => client
                .patch(format!("{kong_url}/services/{}", &service.name))
                .json(&service)
                .send()
                .await
                .map_err(|it| {
                    Errors::UnnableToRegisterApiGateway(format!("Unnable to register service, failed to update {it}"))
                })?,
            _ => resp
        };

        if !resp.status().is_success() {
            return Err(Errors::UnnableToRegisterApiGateway(resp.status().to_string()));
        } else if !resp.status().is_success() {
            return Err(Errors::UnnableToRegisterApiGateway(format!(
                "Failed to update service {} {:?}",
                resp.status(),
                resp.text().await
            )));
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
            return Ok(None);
        }

        let service: GatewayService = resp.json().await.map_err(|it| Errors::GatewayOperationFailed(it.to_string()))?;

        return Ok(Some(service));
    }
}
