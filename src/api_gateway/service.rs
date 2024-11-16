use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GatewayService {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub path: Option<String>,
    pub protocol: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub routes: Vec<GatewayRoute>,
    pub retries: Option<u8>,
    pub connect_timeout: Option<u32>,
    pub write_timeout: Option<u32>,
    pub read_timeout: Option<u32>
}

#[derive(Debug, Default)]
pub struct GatewayServiceBuilder {
    pub name: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub protocol: Option<String>,
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
    pub methods: Option<Vec<String>>,
    pub protocols: Option<Vec<String>>,
    pub strip_path: bool,
    pub preserve_host: bool
}

#[derive(Debug, Default)]
pub struct GatewayRouteBuilder {
    pub name: Option<String>,
    pub paths: Vec<String>,
    pub methods: Option<Vec<String>>,
    pub protocols: Option<Vec<String>>,
    pub strip_path: Option<bool>,
    pub preserve_host: Option<bool>
}

impl GatewayRouteBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
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

    pub fn protocols(mut self, protocols: Vec<String>) -> Self {
        self.protocols = Some(protocols);
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
        self
    }

    pub fn build(self) -> GatewayRoute {
        GatewayRoute {
            name: self.name.expect("Route name is required"),
            paths: self.paths,
            methods: self.methods,
            protocols: self.protocols,
            strip_path: self.strip_path.unwrap_or(false),
            preserve_host: self.preserve_host.unwrap_or(true)
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

    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = Some(host.into());
        self
    }

    pub fn protocol(mut self, protocol: impl Into<String>) -> Self {
        self.protocol = Some(protocol.into());
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
    pub fn grpc(mut self, grpc_host: impl Into<String>, grpc_port: u16) -> Self {
        self.protocol = Some("grpc".to_string());
        self.host = Some(grpc_host.into());
        self.port = Some(grpc_port);
        self
    }

    // Add gRPC support by setting a default gRPC service URL
    pub fn grpc_web(mut self, grpc_host: impl Into<String>, grpc_port: u16) -> Self {
        self.protocol = Some("http".to_string());
        self.host = Some(grpc_host.into());
        self.port = Some(grpc_port);
        self
    }

    pub fn build(self) -> GatewayService {
        GatewayService {
            name: self.name.expect("Service name is required"),
            host: self.host.expect("Service host must be defined"),
            port: self.port.expect("Service port must be defined"),
            path: Some("/".to_owned()),
            protocol: self.protocol.expect("Service protocol must be defined"),
            routes: self.routes.unwrap_or(vec![]),
            retries: self.retries,
            connect_timeout: self.connect_timeout,
            write_timeout: self.write_timeout,
            read_timeout: self.read_timeout
        }
    }
}
