pub mod app;
pub mod errors;
pub mod models;
pub mod storage;

pub type Result<T> = std::result::Result<T, errors::AppError>;
