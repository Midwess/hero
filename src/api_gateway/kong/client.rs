use core_services::services::base::Resolve;
use core_services::services::errors::Errors;
use log::info;
use reqwest::StatusCode;
use serde_json::json;

use crate::api_gateway::client::ApiGatewayClient;
use crate::api_gateway::service::{GatewayRoute, GatewayService};

pub struct KongGatewayAdminClient {
    pub url: String
}

impl KongGatewayAdminClient {
    async fn handle_cors_plugin(&self, route: &GatewayRoute, enable_cors: bool) -> Resolve<()> {
        let kong_url = self.url.clone();
        let client = reqwest::Client::new();

        log::info!(target: "kong-admin", "Enabling CORS plugin for route {:?}", route.name);
        if enable_cors {
            // Add the CORS plugin to the route
            let resp = client
                .post(format!("{kong_url}/routes/{}/plugins", route.name))
                .json(&json!({
                    "name": "cors",
                    "config": {
                        "origins": ["*"], // Allow all origins
                        "methods": [
                            "GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD"
                        ],
                        "headers": [
                            "Accept",
                            "Accept-Encoding",
                            "Authorization",
                            "Content-Type",
                            "X-Grpc-Web",
                            "Grpc-Timeout",
                            "Grpc-Encoding",
                            "Grpc-Accept-Encoding",
                            "User-Agent",
                            "Referer",
                            "Origin"
                        ],
                        "exposed_headers": [
                            "Grpc-Status",
                            "Grpc-Message",
                            "Grpc-Encoding",
                            "Grpc-Accept-Encoding",
                            "Authorization",
                            "Content-Type"
                        ],
                        "max_age": 3600
                    }
                }))
                .send()
                .await
                .map_err(|it| Errors::UnnableToRegisterApiGateway(format!("Unable to enable CORS plugin: {it}")))?;

            if !resp.status().is_success() && StatusCode::CONFLICT != resp.status() {
                return Err(Errors::UnnableToRegisterApiGateway(format!(
                    "Failed to enable CORS plugin: {} {:?}",
                    resp.status(),
                    resp.text().await.unwrap()
                )));
            }
        } else {
            // Get the plugin ID first, then delete it
            let plugins_resp = client
                .get(format!("{kong_url}/routes/{}/plugins", route.name))
                .send()
                .await
                .map_err(|it| Errors::UnnableToRegisterApiGateway(format!("Unable to get plugins: {it}")))?;

            if plugins_resp.status().is_success() {
                let plugins: serde_json::Value = plugins_resp.json().await
                    .map_err(|it| Errors::UnnableToRegisterApiGateway(format!("Unable to parse plugins response: {it}")))?;

                if let Some(plugins_array) = plugins.get("data").and_then(|d| d.as_array()) {
                    for plugin in plugins_array {
                        if let Some(name) = plugin.get("name").and_then(|n| n.as_str()) {
                            if name == "cors" {
                                if let Some(plugin_id) = plugin.get("id").and_then(|id| id.as_str()) {
                                    let _ = client
                                        .delete(format!("{kong_url}/plugins/{}", plugin_id))
                                        .send()
                                        .await
                                        .map_err(|it| Errors::UnnableToRegisterApiGateway(format!(
                                            "Failed to delete CORS plugin {}: {it}", plugin_id
                                        )))?;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_web_plugin(&self, service_name: String, route: &GatewayRoute, enable_web: bool) -> Resolve<()> {
        let kong_url = self.url.clone();
        let client = reqwest::Client::new();

        let mut web_route = route.clone();
        web_route.name = format!("{}_-_web", web_route.name);
        web_route.protocols = Some(vec![
            "http".to_owned(),
            "https".to_owned()
        ]);

        web_route.headers.insert("content-type".to_owned(), vec!["application/grpc-web+proto".to_owned()]);

        if enable_web {
            // Create route for web
            self.create_or_update_route(service_name, &web_route).await?;
            // Add the grpc-web plugin to the route
            let resp = client
                .post(format!("{kong_url}/routes/{}/plugins", web_route.name))
                .json(&json!({
                    "name": "grpc-web"
                }))
                .send()
                .await
                .map_err(|it| Errors::UnnableToRegisterApiGateway(format!("Unable to enable rpc-web plugin: {it}")))?;

            if !resp.status().is_success() && StatusCode::CONFLICT != resp.status() {
                return Err(Errors::UnnableToRegisterApiGateway(format!(
                    "Failed to enable grpc-web plugin: {} {:?}",
                    resp.status(),
                    resp.text().await.unwrap()
                )));
            }
            
            self.handle_cors_plugin(&web_route, true).await?;
        } else {
            // Delete the web route
            let _ = client.delete(format!("{kong_url}/routes/{}", web_route.name)).send().await.map_err(|it| {
                Errors::UnnableToRegisterApiGateway(format!("Failed to delete web grpc route {}: {it}", web_route.name))
            })?;
        }

        Ok(())
    }

    async fn create_or_update_route(&self, service_name: String, route: &GatewayRoute) -> Resolve<()> {
        info!(target: "kong-admin", "Creating route {:?}", json!({"body": route.as_request_body()}));
        let kong_url = self.url.clone();
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("{kong_url}/services/{service_name}/routes"))
            .json(&route.as_request_body())
            .send()
            .await
            .map_err(|it| Errors::UnnableToRegisterApiGateway(format!("Unable to register route {it}")))?;

        let resp = match resp.status() {
            StatusCode::CONFLICT => {
                info!(target: "kong-admin", "Route already exist, change to update {:?}", route.as_request_body());

                client
                    .patch(format!("{kong_url}/services/{service_name}/routes/{}", &route.name))
                    .json(&route.as_request_body())
                    .send()
                    .await
                    .map_err(|it| Errors::UnnableToRegisterApiGateway(format!("Unable to register route {it}")))?
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

        Ok(())
    }
}

#[async_trait::async_trait]
impl ApiGatewayClient for KongGatewayAdminClient {
    async fn route_to(&self, service: GatewayService) -> Resolve<()> {
        for route in service.routes {
            self.create_or_update_route(service.name.clone(), &route).await?;

            self.handle_cors_plugin(&route, route.allow_cross_origin).await?;
            self.handle_web_plugin(service.name.clone(), &route, route.enable_web).await?;
        }

        Ok(())
    }

    async fn register(&self, service: GatewayService) -> Resolve<()> {
        let kong_url = self.url.as_str();
        let client = reqwest::Client::new();

        info!(target: "kong-admin", "Creating service {}", json!({"body": service}));
        let resp = client.post(format!("{kong_url}/services")).json(&service).send().await.map_err(|it| {
            Errors::UnnableToRegisterApiGateway(format!("Unable to register service, failed to create {it}"))
        })?;

        let resp = match resp.status() {
            StatusCode::CONFLICT => client
                .patch(format!("{kong_url}/services/{}", &service.name))
                .json(&service)
                .send()
                .await
                .map_err(|it| {
                    Errors::UnnableToRegisterApiGateway(format!("Unable to register service, failed to update {it}"))
                })?,
            _ => resp
        };

        if !resp.status().is_success() {
            return Err(Errors::UnnableToRegisterApiGateway(format!("{:?}", resp.text().await)));
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
