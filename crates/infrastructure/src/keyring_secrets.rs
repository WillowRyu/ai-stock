use application::ports::secret_store::{SecretError, SecretStore};
use async_trait::async_trait;

pub struct KeyringSecretStore {
    service: String,
}

impl KeyringSecretStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }
}

#[async_trait]
impl SecretStore for KeyringSecretStore {
    async fn get(&self, key: &str) -> Result<String, SecretError> {
        let service = self.service.clone();
        let key = key.to_string();
        tokio::task::spawn_blocking(move || {
            let entry = keyring::Entry::new(&service, &key)
                .map_err(|e| SecretError::Backend(e.to_string()))?;
            match entry.get_password() {
                Ok(v) => Ok(v),
                Err(keyring::Error::NoEntry) => Err(SecretError::NotFound(key)),
                Err(e) => Err(SecretError::Backend(e.to_string())),
            }
        })
        .await
        .map_err(|e| SecretError::Backend(e.to_string()))?
    }

    async fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        let service = self.service.clone();
        let key = key.to_string();
        let value = value.to_string();
        tokio::task::spawn_blocking(move || {
            let entry = keyring::Entry::new(&service, &key)
                .map_err(|e| SecretError::Backend(e.to_string()))?;
            entry
                .set_password(&value)
                .map_err(|e| SecretError::Backend(e.to_string()))
        })
        .await
        .map_err(|e| SecretError::Backend(e.to_string()))?
    }

    async fn delete(&self, key: &str) -> Result<(), SecretError> {
        let service = self.service.clone();
        let key = key.to_string();
        tokio::task::spawn_blocking(move || {
            let entry = keyring::Entry::new(&service, &key)
                .map_err(|e| SecretError::Backend(e.to_string()))?;
            match entry.delete_password() {
                Ok(()) => Ok(()),
                Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(SecretError::Backend(e.to_string())),
            }
        })
        .await
        .map_err(|e| SecretError::Backend(e.to_string()))?
    }
}
