use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GatewayService {
    pub name: String,
    pub url: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub routes: Vec<GatewayRoute>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retries: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connect_timeout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_timeout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_timeout: Option<u32>
}

#[derive(Debug, Default)]
pub struct GatewayServiceBuilder {
    pub name: Option<String>,
    pub url: Option<String>,
    pub routes: Option<Vec<GatewayRoute>>,
    pub retries: Option<u8>,
    pub connect_timeout: Option<u32>,
    pub write_timeout: Option<u32>,
    pub read_timeout: Option<u32>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayRoute {
    pub name: String,
    pub paths: Vec<String>,
    pub hosts: Vec<String>,
    pub regex_priority: i32,
    pub methods: Option<Vec<String>>,
    pub protocols: Option<Vec<String>>,
    pub strip_path: bool,
    pub preserve_host: bool,
    pub headers: HashMap<String, Vec<String>>,
    #[serde(skip_serializing)]
    pub enable_web: bool
}

#[derive(Debug, Default)]
pub struct GatewayRouteBuilder {
    pub name: Option<String>,
    pub paths: Vec<String>,
    pub hosts: Vec<String>,
    pub regex_priority: Option<i32>,
    pub methods: Option<Vec<String>>,
    pub protocols: Option<Vec<String>>,
    pub strip_path: Option<bool>,
    pub preserve_host: Option<bool>,
    pub is_public: Option<bool>,
    pub enable_web: bool
}
impl GatewayRouteBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn enable_web(mut self) -> Self {
        self.enable_web = true;
        self
    }

    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.paths.push(path.into());
        self
    }

    pub fn methods(mut self, methods: Vec<String>) -> Self {
        self.methods = Some(methods);
        self
    }

    pub fn hosts(mut self, hosts: Vec<String>) -> Self {
        self.hosts = hosts;
        self
    }

    pub fn regex_priority(mut self, regex_priority: i32) -> Self {
        self.regex_priority = Some(regex_priority);
        self
    }

    pub fn protocols(mut self, protocols: Vec<String>) -> Self {
        self.protocols = Some(protocols);
        self
    }

    pub fn public(mut self, is_public: bool) -> Self {
        self.is_public = Some(is_public);
        self
    }

    pub fn strip_path(mut self, strip_path: bool) -> Self {
        self.strip_path = Some(strip_path);
        self
    }

    pub fn preserve_host(mut self, preserve_host: bool) -> Self {
        self.preserve_host = Some(preserve_host);
        self
    }

    pub fn grpc(mut self) -> Self {
        self.protocols = Some(vec![
            "grpc".to_string(),
            "grpcs".to_string(),
        ]);

        self
    }

    pub fn grpc_web(mut self) -> Self {
        self.protocols = Some(vec![
            "http".to_string(),
            "https".to_string(),
        ]);

        self.enable_web = true;

        self
    }

    pub fn http(mut self) -> Self {
        self.protocols = Some(vec![
            "http".to_string(),
            "https".to_string(),
        ]);

        self.methods = Some(
            vec!["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD", "CONNECT", "TRACE"]
            .iter().map(|s| s.to_string()).collect()
        );

        self
    }

    pub fn build(self) -> GatewayRoute {
        let mut headers: HashMap<String, Vec<String>> = HashMap::new();
        let is_public = self.is_public.unwrap_or(false);

        if !is_public {
            headers.insert("x-internal".to_owned(), vec!["true".to_owned()]);
        }

        GatewayRoute {
            name: self.name.expect("Route name is required"),
            paths: self.paths,
            methods: self.methods,
            hosts: self.hosts.clone(),
            regex_priority: self.regex_priority.unwrap_or(1),
            protocols: Some(self.protocols.expect("Protocols is required")),
            strip_path: self.strip_path.unwrap_or(false),
            preserve_host: self.preserve_host.unwrap_or(true),
            headers,
            enable_web: self.enable_web
        }
    }
}

impl GatewayServiceBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn routes(mut self, routes: Vec<GatewayRoute>) -> Self {
        self.routes = Some(routes);
        self
    }

    pub fn retries(mut self, retries: u8) -> Self {
        self.retries = Some(retries);
        self
    }

    pub fn connect_timeout(mut self, timeout: u32) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    pub fn write_timeout(mut self, timeout: u32) -> Self {
        self.write_timeout = Some(timeout);
        self
    }

    pub fn read_timeout(mut self, timeout: u32) -> Self {
        self.read_timeout = Some(timeout);
        self
    }

    // Add gRPC support by setting a default gRPC service URL
    pub fn grpc(mut self, grpc_host: String, grpc_port: u16) -> Self {
        self.url = Some(format!("grpc://{grpc_host}:{grpc_port}"));
        self
    }

    pub fn http(mut self, http_host: String, http_port: u16) -> Self {
        self.url = Some(format!("http://{http_host}:{http_port}"));
        self
    }

    pub fn build(self) -> GatewayService {
        GatewayService {
            name: self.name.expect("Service name is required"),
            url: self.url.expect("Service url is required"),
            routes: self.routes.unwrap_or_default(),
            retries: self.retries,
            connect_timeout: self.connect_timeout,
            write_timeout: self.write_timeout,
            read_timeout: self.read_timeout
        }
    }
}
