use chrono::{DateTime, Utc};

#[cfg_attr(test, mockall::automock)]
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}
