use tonic::codegen;
use tonic::{body::BoxBody, Status};
use tonic::codegen::http::{Request, Response};
use tonic::transport::Body;

pub trait GrpcServiceBound : codegen::Service<Request<Body>, Response =
    Response<BoxBody>, Error = Status> + Send + Clone + 'static {}
