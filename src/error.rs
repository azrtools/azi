use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum AppError {
    AccessTokenFileError,
    HttpClientError,
    ServiceError,
    ParseError(String),
}

impl Error for AppError {}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::AccessTokenFileError => f.write_str("Access token file error!"),
            AppError::HttpClientError => f.write_str("HTTP client error!"),
            AppError::ServiceError => f.write_str("Service error!"),
            AppError::ParseError(s) => f.write_str(s),
        }
    }
}
