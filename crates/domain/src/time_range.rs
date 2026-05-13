use chrono::{DateTime, Utc};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TimeRange {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TimeRangeError {
    #[error("end must be >= start")]
    InvalidOrder,
}

impl TimeRange {
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Self, TimeRangeError> {
        if end < start {
            return Err(TimeRangeError::InvalidOrder);
        }
        Ok(Self { start, end })
    }
    pub fn start(&self) -> DateTime<Utc> {
        self.start
    }
    pub fn end(&self) -> DateTime<Utc> {
        self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    #[test]
    fn rejects_inverted_range() {
        let a = Utc.with_ymd_and_hms(2026, 5, 13, 10, 0, 0).unwrap();
        let b = Utc.with_ymd_and_hms(2026, 5, 13, 9, 0, 0).unwrap();
        assert!(TimeRange::new(a, b).is_err());
    }
}
