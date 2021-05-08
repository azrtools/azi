use std::error;
use std::fmt;

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum AppError {
    AccessTokenFileError,
    HttpClientError,
    ServiceError,
    ParseError(String),
    UnexpectedJson(Value),
    InvalidAccessToken(String),
    InvalidTenantId(String),
    MismatchedTenantId(String, String),
    InvalidIssuer(String),
    InvalidAuthority(String),
}

impl error::Error for AppError {}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::AccessTokenFileError => f.write_str("Access token file error!"),
            AppError::HttpClientError => f.write_str("HTTP client error!"),
            AppError::ServiceError => f.write_str("Service error!"),
            AppError::ParseError(s) => f.write_str(s),
            AppError::UnexpectedJson(v) => {
                f.write_fmt(format_args!("Unexpected JSON structure: {:?}", v))
            }
            AppError::InvalidAccessToken(s) => {
                f.write_fmt(format_args!("Invalid access token: {}", s))
            }
            AppError::InvalidTenantId(s) => f.write_fmt(format_args!("Invalid tenant ID: {}", s)),
            AppError::MismatchedTenantId(a, b) => {
                f.write_fmt(format_args!("Mismatched tenant ID: {} != {}", a, b))
            }
            AppError::InvalidIssuer(s) => f.write_fmt(format_args!("Invalid issuer: {}", s)),
            AppError::InvalidAuthority(s) => f.write_fmt(format_args!("Invalid authority: {}", s)),
        }
    }
}
