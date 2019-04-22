use std::error::Error;
use std::ffi::OsString;

pub type Result<T> = std::result::Result<T, Box<Error>>;

pub fn convert_str(os: OsString) -> String {
    match os.as_os_str().to_str() {
        Some(s) => return s.to_string(),
        None => panic!("invalid string: {:?}", os),
    }
}
