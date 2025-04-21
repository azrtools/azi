use std::error::Error;
use std::ffi::OsString;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::path::Path;

use serde::de::DeserializeOwned;
use serde_json::from_reader;
use serde_json::Value;

use crate::error::AppError::UnexpectedJsonType;

const DAYS: &[u32] = &[31, 0, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub fn convert_str(os: OsString) -> String {
    match os.as_os_str().to_str() {
        Some(s) => return s.to_owned(),
        None => panic!("invalid string: {:?}", os),
    }
}

pub fn days_of_month(year: u32, month: u32) -> Result<u32> {
    if month == 2 {
        if year % 4 != 0 || (year % 100 == 0 && year % 400 != 0) {
            return Ok(28);
        } else {
            return Ok(29);
        }
    } else if month > 0 && month <= 12 {
        return Ok(DAYS[(month - 1) as usize]);
    } else {
        return Err(Box::from("invalid month argument!"));
    }
}

pub fn read_file(path: &Path) -> Result<Value> {
    if path.exists() {
        let file = File::open(&path)?;
        let reader = skip_bom(BufReader::new(file))?;
        match from_reader(reader) {
            Err(e) => {
                trace!("Failed to parse file: {}", path.display());
                Err(e.into())
            }
            Ok(value) => Ok(value),
        }
    } else {
        debug!("File not found: {}", path.display());
        Ok(Value::Null)
    }
}

fn skip_bom(mut reader: BufReader<File>) -> Result<BufReader<File>> {
    let buf = reader.fill_buf()?;
    if buf.len() >= 3 && buf[0] == 0xEF && buf[1] == 0xBB && buf[2] == 0xBF {
        reader.read_exact(&mut [0; 3])?;
    }
    Ok(reader)
}

pub trait ValueExt {
    fn to<T: DeserializeOwned>(self) -> Result<T>;
    fn string(&self) -> Result<String>;
    fn to_array(&self) -> Result<&Vec<Value>>;
}

impl ValueExt for Value {
    fn to<T: DeserializeOwned>(self) -> Result<T> {
        Ok(serde_json::from_value(self)?)
    }

    fn string(&self) -> Result<String> {
        self.as_str()
            .ok_or_else(|| UnexpectedJsonType(self.clone(), "string").into())
            .map(&str::to_owned)
    }

    fn to_array(&self) -> Result<&Vec<Value>> {
        self.as_array()
            .ok_or_else(|| UnexpectedJsonType(self.clone(), "array").into())
    }
}

#[cfg(test)]
mod tests {
    use super::days_of_month;

    #[test]
    fn test_days_of_month_feb() {
        assert_eq!(28, days_of_month(2003, 2).unwrap());
        assert_eq!(29, days_of_month(2004, 2).unwrap());
    }

    #[test]
    fn test_days_of_month_jun() {
        assert_eq!(30, days_of_month(2002, 6).unwrap());
    }
}
