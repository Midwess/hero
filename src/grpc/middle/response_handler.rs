use std::str::FromStr;

use tonic::body::BoxBody;
use tonic::codegen::http::{HeaderValue, Request, Response};
use tonic::metadata::MetadataValue;
use tonic::transport::{Body, Server};
use tonic::{async_trait, Status};
use tonic_middleware::{
    InterceptorFor, Middleware, MiddlewareFor, MiddlewareLayer, RequestInterceptor,
    RequestInterceptorLayer, ServiceBound,
};

#[derive(Default, Clone)]
pub struct ResponseHeaderHandler;

#[async_trait]
impl<S> Middleware<S> for ResponseHeaderHandler
where
    S: ServiceBound,
    S::Future: Send,
{
    async fn call(
        &self,
        req: Request<Body>,
        mut service: S,
    ) -> Result<Response<BoxBody>, S::Error> {
        let mut result = service.call(req).await?;

        result.headers_mut().insert("Access-Control-Expose-Headers", HeaderValue::from_str("*").unwrap());
        Ok(result)
    }
}
