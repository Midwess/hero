use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum Errors {
    #[error("Operation on db name {} got error {}", .db_name, .message)]
    DatabaseError { message: String, db_name: String},
    #[error("User {} already exist", .0)]
    UserAlreadyExist(String),
    #[error("Failed to generate auth token {:?}", .0)]
    GenerateTokenFailed(#[from] jsonwebtoken::errors::Error),
    #[error("UnAuthorize {}", .0)]
    UnAuthorized(String)
}

impl From<surrealdb::Error> for Errors {
    fn from(value: surrealdb::Error) -> Self {
        Self::DatabaseError { message: value.to_string(), db_name: "surrealdb".to_owned() }
    }
}
