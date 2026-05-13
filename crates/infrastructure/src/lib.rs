//! Adapters implementing application ports. Only place where reqwest/sqlx/keyring live.
pub mod clock;
pub mod http;
pub mod keyring_secrets;
pub mod providers;
pub mod sqlite;
