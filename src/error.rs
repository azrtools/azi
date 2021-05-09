use std::error;
use std::fmt;

use serde_json::Value;

#[derive(Debug)]
pub enum AppError {
    AccessTokenFileError,
    HttpClientError,
    ServiceError(&'static str),

    ParseError(String),

    HttpError(u16, Value),
    InvalidCertificate(String),

    UnexpectedJson(Value),
    UnexpectedJsonType(Value, &'static str),

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
            AppError::ServiceError(s) => f.write_str(s),
            AppError::ParseError(s) => f.write_str(s),
            AppError::HttpError(status, _) => f.write_fmt(format_args!("HTTP error {}", status)),
            AppError::InvalidCertificate(cert) => {
                f.write_fmt(format_args!("Invalid certificate data: {}", cert))
            }
            AppError::UnexpectedJson(json) => {
                f.write_fmt(format_args!("Unexpected JSON structure: {:?}", json))
            }
            AppError::UnexpectedJsonType(json, t) => {
                f.write_fmt(format_args!("Unexpected JSON, expected {}: {:?}", t, json))
            }
            AppError::InvalidAccessToken(token) => {
                f.write_fmt(format_args!("Invalid access token: {}", token))
            }
            AppError::InvalidTenantId(id) => f.write_fmt(format_args!("Invalid tenant ID: {}", id)),
            AppError::MismatchedTenantId(a, b) => {
                f.write_fmt(format_args!("Mismatched tenant ID: {} != {}", a, b))
            }
            AppError::InvalidIssuer(issuer) => {
                f.write_fmt(format_args!("Invalid issuer: {}", issuer))
            }
            AppError::InvalidAuthority(authority) => {
                f.write_fmt(format_args!("Invalid authority: {}", authority))
            }
        }
    }
}
