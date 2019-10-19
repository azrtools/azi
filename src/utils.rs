use std::error::Error;
use std::ffi::OsString;

const DAYS: &[u32] = &[31, 0, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

pub type Result<T> = std::result::Result<T, Box<Error>>;

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
