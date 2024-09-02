use tonic::body::BoxBody;
use tonic::codegen::http::{Request, Response};
use tonic::transport::Body;
use tonic::{codegen, Status};

pub trait GrpcServiceBound:
    codegen::Service<Request<Body>, Response = Response<BoxBody>, Error = Status> + Send + Clone + 'static
{
}
