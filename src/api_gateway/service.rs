//! Declarative Kong service and expression-route definitions.
//!
//! Builders encode Hero's gateway conventions: HTTP and gRPC protocol groups,
//! internal routes restricted to Kong's internal listener, optional gRPC-Web
//! companion routes, and service-level plugins.

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
/// Validation failure while building a gateway definition.
pub enum BuildError {
    /// A field required to identify or route the resource was not configured.
    #[error("missing required field `{0}`")]
    MissingField(&'static str),
}

#[derive(Debug, Serialize, Deserialize)]
/// Desired gateway state for one upstream service.
pub struct GatewayService {
    /// Stable Kong service name.
    pub name: String,
    /// Upstream URL, such as `grpc://service:50051` or `http://service:3000`.
    pub url: String,
    #[serde(skip_serializing, skip_deserializing)]
    /// Routes reconciled after the service itself.
    pub routes: Vec<GatewayRoute>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Kong upstream retry count.
    pub retries: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Kong upstream connection timeout in milliseconds.
    pub connect_timeout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Kong upstream write timeout in milliseconds.
    pub write_timeout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Kong upstream read timeout in milliseconds.
    pub read_timeout: Option<u32>,
    #[serde(skip_serializing, skip_deserializing)]
    /// Whether reconciliation enables Kong's CORS plugin.
    pub enable_cors: bool,
    #[serde(skip_serializing, skip_deserializing)]
    /// Headers added by Kong before forwarding upstream.
    pub request_headers: HashMap<String, String>,
}

impl GatewayService {
    /// Produces a compact description suitable for startup logs.
    pub fn log_summary(&self) -> String {
        format!("Service {} with {} route(s)", self.name, self.routes.len())
    }
}

#[derive(Debug, Default)]
/// Incremental builder for [`GatewayService`].
pub struct GatewayServiceBuilder {
    /// Pending service name.
    pub name: Option<String>,
    /// Pending upstream URL.
    pub url: Option<String>,
    /// Pending route collection.
    pub routes: Option<Vec<GatewayRoute>>,
    /// Pending retry count.
    pub retries: Option<u8>,
    /// Pending connection timeout in milliseconds.
    pub connect_timeout: Option<u32>,
    /// Pending write timeout in milliseconds.
    pub write_timeout: Option<u32>,
    /// Pending read timeout in milliseconds.
    pub read_timeout: Option<u32>,
    /// Pending CORS-plugin state.
    pub enable_cors: bool,
    /// Pending upstream request headers.
    pub request_headers: HashMap<String, String>,
}

#[derive(Debug, Clone)]
/// A prebuilt regular expression for Kong's expression router.
pub struct GatewayRouteExpression(String);

impl From<GatewayRouteExpression> for String {
    fn from(val: GatewayRouteExpression) -> Self {
        val.0
    }
}

impl GatewayRouteExpression {
    /// Matches a protobuf namespace at the beginning of the request path.
    pub fn proto_namespace(proto_namespace: &str) -> Self {
        let expr = format!(r#"^/{proto_namespace}"#);
        Self(expr)
    }

    /// Matches and captures a protobuf namespace below a public prefix.
    pub fn proto_namespace_capture(prefix: &str, proto_namespace: &str) -> Self {
        let expr = format!(r#"^/{prefix}/({proto_namespace}/.*)"#);
        Self(expr)
    }

    /// Matches any path beginning with the escaped literal path.
    pub fn start_with(path: &str) -> Self {
        let expr = format!(r#"^{}"#, regex::escape(path));
        Self(expr)
    }

    /// Matches a literal path exactly or any child path below it.
    pub fn exact_or_subpath(path: &str) -> Self {
        let expr = format!(r#"^{}(?:/|$)"#, regex::escape(path));
        Self(expr)
    }
}

#[derive(Debug, Clone)]
/// Desired Kong expression route and its Hero-managed plugin behavior.
pub struct GatewayRoute {
    /// Stable route name.
    pub name: String,
    /// Regular-expression path predicates.
    pub paths: Vec<String>,
    /// Exact host predicates.
    pub hosts: Vec<String>,
    /// Optional host regular expression, taking precedence over exact hosts.
    pub host_regex: Option<String>,
    /// Kong expression-router priority.
    pub priority: u32,
    /// Optional allowed HTTP methods.
    pub methods: Option<Vec<String>>,
    /// Kong protocols accepted by this route.
    pub protocols: Option<Vec<String>>,
    /// Whether Kong strips the matched path before proxying.
    pub strip_path: bool,
    /// Whether Kong forwards the original `Host` header.
    pub preserve_host: bool,
    /// Header predicates included in the route expression.
    pub headers: HashMap<String, Vec<String>>,
    /// Whether the route is reachable through the public listener.
    pub is_public: bool,
    /// Whether reconciliation creates the gRPC-Web companion route and plugin.
    pub enable_grpc_web: bool,
    /// Kong request buffering setting.
    pub request_buffering: bool,
    /// Kong response buffering setting.
    pub response_buffering: bool,
}

impl GatewayRoute {
    /// Encodes the route fields accepted by Kong's expression-router API.
    pub fn as_request_body(&self) -> serde_json::Value {
        // When using Kong expression router, headers are included in the expression
        // via build_expression() - do NOT add separate "headers" field
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

    /// Builds the Kong expression joining configured predicates with `&&`.
    ///
    /// Internal routes add `net.dst.port == 8000`. Empty predicate groups are
    /// omitted, and multiple values within one group are joined with `||`.
    pub fn build_expression(&self) -> String {
        let mut parts = vec![];

        if let Some(regex) = &self.host_regex {
            parts.push(format!(r##"http.host ~ r#"{regex}"#"##));
        } else if !self.hosts.is_empty() {
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

        // Port-based routing for internal routes
        // When is_public=false, require net.dst.port == 8000 (internal port)
        if !self.is_public {
            parts.push("net.dst.port == 8000".to_string());
        }

        parts.join(" && ")
    }

    /// Produces a compact description suitable for startup logs.
    pub fn log_summary(&self) -> String {
        let path_count = self.paths.len();
        let host_count = self.hosts.len();
        let method_count = self.methods.as_ref().map(|m| m.len()).unwrap_or(0);
        let header_count = self.headers.len();

        format!(
            "Route {} ({} paths, {} hosts, {} methods, {} headers)",
            self.name, path_count, host_count, method_count, header_count
        )
    }
}

#[derive(Debug, Default)]
/// Incremental builder for [`GatewayRoute`].
pub struct GatewayRouteBuilder {
    /// Pending route name.
    pub name: Option<String>,
    /// Pending regular-expression path predicates.
    pub paths: Vec<String>,
    /// Pending exact host predicates.
    pub hosts: Vec<String>,
    /// Pending host regular expression.
    pub host_regex: Option<String>,
    /// Pending expression priority.
    pub priority: Option<u32>,
    /// Pending HTTP methods.
    pub methods: Option<Vec<String>>,
    /// Pending Kong protocols.
    pub protocols: Option<Vec<String>>,
    /// Pending path-stripping behavior.
    pub strip_path: Option<bool>,
    /// Pending host-preservation behavior.
    pub preserve_host: Option<bool>,
    /// Pending public/internal visibility.
    pub is_public: Option<bool>,
    /// Whether gRPC header predicates should be added during build.
    pub is_grpc: bool,
    /// Whether a gRPC-Web companion route should be reconciled.
    pub enable_grpc_web: bool,
    /// Reserved web CORS state for compatibility with existing definitions.
    pub enable_web_cors: bool,
    /// Pending request-buffering behavior.
    pub request_buffering: bool,
    /// Pending response-buffering behavior.
    pub response_buffering: bool,
    /// Pending header predicates.
    pub headers: HashMap<String, Vec<String>>,
}

impl GatewayRouteBuilder {
    /// Starts a route with Hero's public, preserve-host defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the stable name used to reconcile the route in Kong.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Controls whether Kong buffers the complete downstream request.
    pub fn request_buffering(mut self, v: bool) -> Self {
        self.request_buffering = v;
        self
    }

    /// Controls whether Kong buffers the complete upstream response.
    pub fn response_buffering(mut self, v: bool) -> Self {
        self.response_buffering = v;
        self
    }

    /// Adds a regular-expression predicate for the request path.
    ///
    /// Use [`GatewayRouteExpression`] helpers when the input begins as a literal
    /// path rather than a Kong-compatible regular expression.
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.paths.push(path.into());
        self
    }

    /// Restricts the route to an explicit set of HTTP methods.
    pub fn methods(mut self, methods: Vec<String>) -> Self {
        self.methods = Some(methods);
        self
    }

    /// Restricts the route to exact host names.
    pub fn hosts(mut self, hosts: Vec<String>) -> Self {
        self.hosts = hosts;
        self
    }

    /// Uses a Kong regular expression for host matching.
    ///
    /// A host expression takes precedence over values configured with
    /// [`GatewayRouteBuilder::hosts`].
    pub fn host_regex(mut self, regex: impl Into<String>) -> Self {
        self.host_regex = Some(regex.into());
        self
    }

    /// Sets the expression-router priority used to break overlapping matches.
    pub fn priority(mut self, priority: u32) -> Self {
        self.priority = Some(priority);
        self
    }

    /// Sets the Kong protocols accepted by the route.
    ///
    /// Prefer [`GatewayRouteBuilder::http`] or [`GatewayRouteBuilder::grpc`] for
    /// the conventional protocol groups.
    pub fn protocols(mut self, protocols: Vec<String>) -> Self {
        self.protocols = Some(protocols);
        self
    }

    /// Chooses between the public listener and Hero's internal port predicate.
    pub fn public(mut self, is_public: bool) -> Self {
        self.is_public = Some(is_public);
        self
    }

    /// Controls whether Kong removes the matched path prefix upstream.
    pub fn strip_path(mut self, strip_path: bool) -> Self {
        self.strip_path = Some(strip_path);
        self
    }

    /// Controls whether the original request host is forwarded upstream.
    pub fn preserve_host(mut self, preserve_host: bool) -> Self {
        self.preserve_host = Some(preserve_host);
        self
    }

    /// Adds an accepted value for a request-header predicate.
    ///
    /// Repeating a key creates an `OR` group for that header.
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let value = value.into();
        self.headers.entry(key).or_default().push(value);
        self
    }

    /// Applies Hero's native gRPC route defaults.
    ///
    /// This selects `grpc`/`grpcs`, disables buffering, and adds the standard
    /// gRPC content-type predicates during build.
    pub fn grpc(mut self) -> Self {
        self.protocols = Some(vec!["grpc".to_string(), "grpcs".to_string()]);

        self.request_buffering = false;
        self.response_buffering = false;
        self.is_grpc = true;

        self
    }

    /// Requests a companion HTTP route with Kong's gRPC-Web plugin.
    pub fn grpc_web(mut self) -> Self {
        self.enable_grpc_web = true;
        self.request_buffering = false;
        self.response_buffering = false;
        self
    }

    /// Applies Hero's HTTP route defaults and all common HTTP methods.
    pub fn http(mut self) -> Self {
        self.protocols = Some(vec!["http".to_string(), "https".to_string()]);

        self.methods = Some(
            [
                "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "CONNECT", "TRACE", "OPTIONS",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        );

        self
    }

    /// Validates required fields and freezes the route definition.
    ///
    /// A route requires a name and protocols. Other fields use Hero's gateway
    /// defaults, including public visibility and host preservation.
    pub fn build(self) -> Result<GatewayRoute, BuildError> {
        let mut headers: HashMap<String, Vec<String>> = HashMap::new();

        if self.is_grpc {
            headers.insert(
                "content-type".to_owned(),
                vec![
                    "application/grpc".to_owned(),
                    "application/grpc+proto".to_owned(),
                    "application/grpc+json".to_owned(),
                ],
            );
        }

        for (key, values) in self.headers {
            headers.entry(key).or_default().extend(values);
        }

        log::debug!(
            "Registering route {} with {} header(s)",
            self.name.as_deref().unwrap_or("unnamed"),
            headers.len()
        );
        Ok(GatewayRoute {
            name: self.name.ok_or(BuildError::MissingField("name"))?,
            paths: self.paths,
            methods: self.methods,
            hosts: self.hosts.clone(),
            host_regex: self.host_regex,
            priority: self.priority.unwrap_or(0),
            protocols: Some(self.protocols.ok_or(BuildError::MissingField("protocols"))?),
            strip_path: self.strip_path.unwrap_or(false),
            preserve_host: self.preserve_host.unwrap_or(true),
            headers,
            is_public: self.is_public.unwrap_or(true),
            enable_grpc_web: self.enable_grpc_web,
            response_buffering: self.response_buffering,
            request_buffering: self.request_buffering,
        })
    }
}

impl GatewayServiceBuilder {
    /// Starts an empty service definition.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the Kong service name after replacing unsupported characters with `-`.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        let name: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '~') {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        self.name = Some(name);
        self
    }

    /// Controls reconciliation of Kong's CORS plugin for the service.
    pub fn enable_cors(mut self, enable: bool) -> Self {
        self.enable_cors = enable;
        self
    }

    /// Adds a header that Kong injects before forwarding requests upstream.
    pub fn request_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.request_headers.insert(key.into(), value.into());
        self
    }

    /// Replaces the service's desired route collection.
    pub fn routes(mut self, routes: Vec<GatewayRoute>) -> Self {
        self.routes = Some(routes);
        self
    }

    /// Sets Kong's retry count for upstream failures.
    pub fn retries(mut self, retries: u8) -> Self {
        self.retries = Some(retries);
        self
    }

    /// Sets the upstream connection timeout in milliseconds.
    pub fn connect_timeout(mut self, timeout: u32) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    /// Sets the upstream write timeout in milliseconds.
    pub fn write_timeout(mut self, timeout: u32) -> Self {
        self.write_timeout = Some(timeout);
        self
    }

    /// Sets the upstream read timeout in milliseconds.
    pub fn read_timeout(mut self, timeout: u32) -> Self {
        self.read_timeout = Some(timeout);
        self
    }

    /// Sets a cleartext HTTP/2 gRPC upstream URL.
    ///
    /// Kong requires the `grpc://` scheme to proxy h2c traffic correctly.
    pub fn grpc(mut self, grpc_host: String, grpc_port: u16) -> Self {
        self.url = Some(format!("grpc://{grpc_host}:{grpc_port}"));
        self
    }

    /// Sets a plain HTTP upstream URL.
    pub fn http(mut self, http_host: String, http_port: u16) -> Self {
        self.url = Some(format!("http://{http_host}:{http_port}"));
        self
    }

    /// Sets an explicit upstream URL for protocols not covered by the helpers.
    pub fn url(mut self, url: String) -> Self {
        self.url = Some(url);
        self
    }

    /// Validates the service identity and upstream URL.
    ///
    /// Routes default to an empty collection; optional Kong transport settings
    /// are omitted when not configured.
    pub fn build(self) -> Result<GatewayService, BuildError> {
        Ok(GatewayService {
            name: self.name.ok_or(BuildError::MissingField("name"))?,
            url: self.url.ok_or(BuildError::MissingField("url"))?,
            routes: self.routes.unwrap_or_default(),
            retries: self.retries,
            connect_timeout: self.connect_timeout,
            write_timeout: self.write_timeout,
            read_timeout: self.read_timeout,
            enable_cors: self.enable_cors,
            request_headers: self.request_headers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_builder_reports_missing_fields() {
        assert_eq!(
            GatewayRouteBuilder::new().http().build().unwrap_err(),
            BuildError::MissingField("name")
        );
        assert_eq!(
            GatewayRouteBuilder::new().name("route").build().unwrap_err(),
            BuildError::MissingField("protocols")
        );
    }

    #[test]
    fn route_expression_combines_path_method_and_visibility() {
        let route = GatewayRouteBuilder::new()
            .name("users")
            .path(GatewayRouteExpression::exact_or_subpath("/users"))
            .methods(vec!["GET".to_owned()])
            .protocols(vec!["http".to_owned()])
            .public(false)
            .build()
            .unwrap();

        assert_eq!(
            route.build_expression(),
            r##"http.path ~ r#"^/users(?:/|$)"# && http.method == "GET" && net.dst.port == 8000"##
        );
    }

    #[test]
    fn service_builder_sanitizes_name() {
        let service = GatewayServiceBuilder::new()
            .name("user service")
            .url("http://localhost:3000".to_owned())
            .build()
            .unwrap();
        assert_eq!(service.name, "user-service");
    }
}
