use aws_sdk_s3::error::SdkError;
use thiserror::Error;
use tonic::Status;

#[derive(Error, Debug)]
pub enum Errors {
    #[error("Operation on db name {} got error {}", .db_name, .message)]
    DatabaseError { message: String, db_name: String},
    #[error("User {} already exist", .0)]
    UserAlreadyExist(String),
    #[error("Failed to generate auth token {:?}", .0)]
    GenerateTokenFailed(#[from] jsonwebtoken::errors::Error),
    #[error("UnAuthorize {}", .0)]
    UnAuthorized(String),
    #[error("Resource got error {:?}", .0)]
    AwsS3Error(String)
}

impl From<surrealdb::Error> for Errors {
    fn from(value: surrealdb::Error) -> Self {
        Self::DatabaseError { message: value.to_string(), db_name: "surrealdb".to_owned() }
    }
}

impl <E, R> From<SdkError<E, R>> for Errors {
    fn from(value: SdkError<E, R>) -> Self {
        return Self::AwsS3Error(value.to_string())
    }
}

impl From<Errors> for Status {
    fn from(value: Errors) -> Self {
        match value {
            Errors::DatabaseError { message, db_name } => Self::aborted(message),
            Errors::UserAlreadyExist(_) => Self::already_exists("User already exist".to_owned()),
            Errors::GenerateTokenFailed(_) => Self::unauthenticated("Invalid token"),
            Errors::UnAuthorized(_) => Self::unauthenticated("You are not allow to access this operation"),
            Errors::AwsS3Error(_) => Self::aborted("Resource busy, please contact support"),
        }
    }
}
