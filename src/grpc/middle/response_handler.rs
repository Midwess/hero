use tonic::body::BoxBody;
use tonic::codegen::http::{HeaderValue, Request, Response};
use tonic::transport::Body;
use tonic::async_trait;
use tonic_middleware::{
    Middleware, ServiceBound,
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
