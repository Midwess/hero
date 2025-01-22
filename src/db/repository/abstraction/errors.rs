pub type Resolve<T> = Result<T, RepositoryError>;
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("Not found {0} {1}")]
    NotFound(String, String),
    #[error("Conflict {0} {1}")]
    Conflict(String, String),
    #[error("Error on db side {0}")]
    SurrealDbError(#[from] surrealdb::Error),
    #[error("Error on db side {0}")]
    DbError(String)
}
