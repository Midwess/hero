use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

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
    pub read_timeout: Option<u32>,
    #[serde(skip_serializing, skip_deserializing)]
    pub enable_cors: bool
}

#[derive(Debug, Default)]
pub struct GatewayServiceBuilder {
    pub name: Option<String>,
    pub url: Option<String>,
    pub routes: Option<Vec<GatewayRoute>>,
    pub retries: Option<u8>,
    pub connect_timeout: Option<u32>,
    pub write_timeout: Option<u32>,
    pub read_timeout: Option<u32>,
    pub enable_cors: bool
}

#[derive(Debug, Clone)]
pub struct GatewayRouteExpression(String);

impl From<GatewayRouteExpression> for String {
    fn from(val: GatewayRouteExpression) -> Self {
        val.0
    }
}

impl GatewayRouteExpression {
    pub fn proto_namespace(proto_namespace: &str) -> Self {
        let expr = format!(r#"^/{proto_namespace}"#,);
        Self(expr)
    }

    pub fn start_with(path: &str) -> Self {
        let expr = format!(r#"^{}"#, regex::escape(path));
        Self(expr)
    }
}

#[derive(Debug, Clone)]
pub struct GatewayRoute {
    pub name: String,
    pub paths: Vec<String>,
    pub hosts: Vec<String>,
    pub priority: u32,
    pub methods: Option<Vec<String>>,
    pub protocols: Option<Vec<String>>,
    pub strip_path: bool,
    pub preserve_host: bool,
    pub headers: HashMap<String, Vec<String>>,
    pub enable_grpc_web: bool,
    pub request_buffering: bool,
    pub response_buffering: bool,
}

impl GatewayRoute {
    pub fn as_request_body(&self) -> serde_json::Value {
        json! ({
            "name": self.name,
            "priority": self.priority,
            "strip_path": self.strip_path,
            "protocols": self.protocols,
            "preserve_host": self.preserve_host,
            "expression": self.build_expression(),
            "request_buffering": self.request_buffering,
            "response_buffering": self.response_buffering,
        })
    }

    pub fn build_expression(&self) -> String {
        let mut parts = vec![];

        // Hosts
        if !self.hosts.is_empty() {
            if self.hosts.len() == 1 {
                parts.push(format!("http.host == \"{}\"", self.hosts[0]));
            } else {
                let host_expr =
                    self.hosts.iter().map(|h| format!("http.host == \"{}\"", h)).collect::<Vec<_>>().join(" || ");
                parts.push(format!("({})", host_expr));
            }
        }

        // Paths
        if !self.paths.is_empty() {
            if self.paths.len() == 1 {
                let path = &self.paths[0];
                parts.push(format!(r##"http.path ~ r#"{path}"#"##));
            } else {
                let path_expr =
                    self.paths.iter().map(|p| format!(r#"http.path ~ "{p}""#)).collect::<Vec<_>>().join(" || ");
                parts.push(format!("({})", path_expr));
            }
        }

        // Methods
        if let Some(methods) = &self.methods {
            if methods.len() == 1 {
                parts.push(format!("http.method == \"{}\"", methods[0]));
            } else if !methods.is_empty() {
                let method_expr =
                    methods.iter().map(|m| format!("http.method == \"{}\"", m)).collect::<Vec<_>>().join(" || ");
                parts.push(format!("({})", method_expr));
            }
        }

        // Headers
        for (key, values) in &self.headers {
            let key = key.replace("-", "_");
            if values.len() == 1 {
                parts.push(format!(r##"http.headers.{key} == "{}""##, values[0]));
            } else if !values.is_empty() {
                let header_expr =
                    values.iter().map(|v| format!(r##"http.headers.{key} == "{v}""##)).collect::<Vec<_>>().join(" || ");
                parts.push(format!("({})", header_expr));
            }
        }

        parts.join(" && ")
    }
}

#[derive(Debug, Default)]
pub struct GatewayRouteBuilder {
    pub name: Option<String>,
    pub paths: Vec<String>,
    pub hosts: Vec<String>,
    pub priority: Option<u32>,
    pub methods: Option<Vec<String>>,
    pub protocols: Option<Vec<String>>,
    pub strip_path: Option<bool>,
    pub preserve_host: Option<bool>,
    pub is_public: Option<bool>,
    pub is_grpc: bool,
    pub enable_grpc_web: bool,
    pub enable_web_cors: bool,
    pub request_buffering: bool,
    pub response_buffering: bool,
}
impl GatewayRouteBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn request_buffering(mut self, v: bool) -> Self {
        self.request_buffering = v;
        self
    }

    pub fn response_buffering(mut self, v: bool) -> Self {
        self.response_buffering = v;
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

    pub fn priority(mut self, priority: u32) -> Self {
        self.priority = Some(priority);
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

        self.request_buffering = false;
        self.response_buffering = false;
        self.is_grpc = true;

        self
    }

    pub fn grpc_web(mut self) -> Self {
        self.enable_grpc_web = true;

        self
    }

    pub fn http(mut self) -> Self {
        self.protocols = Some(vec![
            "http".to_string(),
            "https".to_string(),
            "http2".to_string(),
            "https+http2".to_string(),
        ]);

        self.methods = Some(
            [
                "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "CONNECT", "TRACE", "OPTIONS"
            ]
            .iter()
            .map(|s| s.to_string())
            .collect()
        );

        self
    }

    pub fn build(self) -> GatewayRoute {
        let mut headers: HashMap<String, Vec<String>> = HashMap::new();
        let is_public = self.is_public.unwrap_or(false);

        if !is_public {
            headers.insert("xinternal".to_owned(), vec!["true".to_owned()]);
        }

        if self.is_grpc {
            headers.insert("content-type".to_owned(), vec!["application/grpc".to_owned()]);
        }

        GatewayRoute {
            name: self.name.expect("Route name is required"),
            paths: self.paths,
            methods: self.methods,
            hosts: self.hosts.clone(),
            priority: self.priority.unwrap_or(0),
            protocols: Some(self.protocols.expect("Protocols is required")),
            strip_path: self.strip_path.unwrap_or(false),
            preserve_host: self.preserve_host.unwrap_or(true),
            headers,
            enable_grpc_web: self.enable_grpc_web,
            response_buffering: self.response_buffering,
            request_buffering: self.request_buffering,
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

    pub fn enable_cors(mut self, enable: bool) -> Self {
        self.enable_cors = enable;
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
            read_timeout: self.read_timeout,
            enable_cors: self.enable_cors
        }
    }
}
