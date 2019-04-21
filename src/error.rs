use std::error::Error;
use std::fmt;

#[derive(Debug, Clone)]
pub enum AppError {
    AccessTokenFileError,
    HttpClientError,
    ServiceError,
}

impl Error for AppError {
    fn description(&self) -> &str {
        match *self {
            AppError::AccessTokenFileError => "Access token file error!",
            AppError::HttpClientError => "HTTP client error!",
            AppError::ServiceError => "Service error!",
        }
    }

    fn cause(&self) -> Option<&Error> {
        None
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}
