#[cfg(feature = "s3")]
use aws_sdk_s3::error::SdkError;
#[cfg(feature = "s3")]
use aws_sdk_s3::presigning::PresigningConfigError;
#[cfg(any(feature = "db-local", feature = "db-remote"))]
use surreal_devl::surreal_qr::SurrealResponseError;
use thiserror::Error;
#[cfg(feature = "grpc-server")]
use tonic::Status;

#[cfg(any(feature = "db-local", feature = "db-remote"))]
use crate::db::repository::abstraction::errors::RepositoryError;

#[derive(Error, Debug)]
pub enum Errors {
    #[error("Operation on db name {} got error {}", .db_name, .message)]
    DatabaseError { message: String, db_name: String },
    #[error("Bad request {}", .0)]
    BadRequest(String),
    #[error("User {} already exist", .0)]
    UserAlreadyExist(String),
    #[error("Already exist {}", .0)]
    AlreadyExist(String),
    #[cfg(feature = "grpc-server")]
    #[error("Failed to generate auth token {:?}", .0)]
    GenerateTokenFailed(#[from] jsonwebtoken::errors::Error),
    #[error("UnAuthorize {}", .0)]
    UnAuthorized(String),
    #[error("Resource got error {:?}", .0)]
    AwsS3Error(String),
    #[error("Unexpected data format {}", .0)]
    UnexpectedDataFormat(String),
    #[error("Resource not found {}", .0)]
    ResourceNotFound(String),
    #[error("Failed to send email {}", .0)]
    FailedToSendEmail(String),
    #[error("IO Error, {}", .0)]
    IoError(#[from] std::io::Error),
    #[error("All resource is busy")]
    ResourceBusy(),
    #[error("Unsupported resource")]
    UnSupportedResource(),
    #[error("Failed to register service to core_api gateway {}", .0)]
    UnnableToRegisterApiGateway(String),
    #[error("Gateway operation failed {}", .0)]
    GatewayOperationFailed(String),
    #[error("Tcp failure {}", .0)]
    TcpFailure(String),
    #[error("Failed to download file {}", .0)]
    FailedToDownloadFile(String),
    #[error("Invalid s3 path format {}", .0)]
    InvalidS3Path(String),
    #[cfg(any(feature = "db-local", feature = "db-remote"))]
    #[error("Repository error {}", .0)]
    RepositoryError(#[from] RepositoryError),
    #[error("Json error {}", .0)]
    JsonError(#[from] serde_json::Error)
}

#[cfg(any(feature = "db-local", feature = "db-remote"))]
impl From<surrealdb::Error> for Errors {
    fn from(value: surrealdb::Error) -> Self {
        Self::DatabaseError {
            message: value.to_string(),
            db_name: "surrealdb".to_owned()
        }
    }
}

#[cfg(any(feature = "db-local", feature = "db-remote"))]
impl From<SurrealResponseError> for Errors {
    fn from(value: SurrealResponseError) -> Self {
        Self::UnexpectedDataFormat(format!("{:?}", &value))
    }
}

#[cfg(feature = "s3")]
impl<E, R> From<SdkError<E, R>> for Errors {
    fn from(value: SdkError<E, R>) -> Self {
        Self::AwsS3Error(value.to_string())
    }
}

#[cfg(feature = "s3")]
impl From<PresigningConfigError> for Errors {
    fn from(value: PresigningConfigError) -> Self {
        Self::AwsS3Error(value.to_string())
    }
}

#[cfg(feature = "grpc-server")]
impl From<Errors> for Status {
    fn from(value: Errors) -> Self {
        match value {
            Errors::DatabaseError { message, db_name: _ } => Self::aborted(message),
            Errors::UserAlreadyExist(_) => Self::already_exists("User already exist".to_owned()),
            #[cfg(feature = "grpc-server")]
            Errors::GenerateTokenFailed(_) => Self::unauthenticated("Invalid token"),
            Errors::UnAuthorized(_) => Self::unauthenticated("You are not allow to access this operation"),
            Errors::AwsS3Error(message) => Self::aborted(format!("Resource busy, please contact support {message}")),
            Errors::BadRequest(message) => Self::invalid_argument(message),
            Errors::UnexpectedDataFormat(_) => Self::aborted("Internal error, contact support"),
            Errors::AlreadyExist(message) => Self::already_exists(format!("Already exist {message}")),
            Errors::ResourceNotFound(message) => Self::not_found(message.to_owned()),
            Errors::FailedToSendEmail(_) => Self::internal("Failed to send email"),
            Errors::IoError(_) => Self::internal("Internal error"),
            Errors::ResourceBusy() => Self::internal("Not resource left, please try again later"),
            Errors::UnSupportedResource() => Self::internal("Unsupported resource"),
            Errors::UnnableToRegisterApiGateway(_) => Self::internal("Service failed to start"),
            Errors::GatewayOperationFailed(_) => Self::internal("Service failed to start"),
            Errors::TcpFailure(_) => Self::internal("Service failed to start"),
            Errors::FailedToDownloadFile(_) => Self::internal("Failed to download file"),
            Errors::InvalidS3Path(_) => Self::invalid_argument("Invalid s3 path"),
            #[cfg(any(feature = "db-local", feature = "db-remote"))]
            Errors::RepositoryError(_) => Self::internal("Internal error, contact support"),
            Errors::JsonError(_) => Self::internal("Internal error, contact support")
        }
    }
}
