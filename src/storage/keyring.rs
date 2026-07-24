use crate::error::LimitLaneError;
use keyring::Entry;

pub struct KeyringStore;

impl KeyringStore {
    pub fn store_secret(service: &str, username: &str, secret: &str) -> Result<(), LimitLaneError> {
        let entry = Entry::new(service, username)
            .map_err(|e| LimitLaneError::CredentialStore(e.to_string()))?;
        entry
            .set_password(secret)
            .map_err(|e| LimitLaneError::CredentialStore(e.to_string()))?;
        Ok(())
    }

    pub fn get_secret(service: &str, username: &str) -> Result<Option<String>, LimitLaneError> {
        let entry = Entry::new(service, username)
            .map_err(|e| LimitLaneError::CredentialStore(e.to_string()))?;
        match entry.get_password() {
            Ok(pwd) => Ok(Some(pwd)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(LimitLaneError::CredentialStore(e.to_string())),
        }
    }

    pub fn delete_secret(service: &str, username: &str) -> Result<(), LimitLaneError> {
        let entry = Entry::new(service, username)
            .map_err(|e| LimitLaneError::CredentialStore(e.to_string()))?;
        match entry.delete_password() {
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(LimitLaneError::CredentialStore(e.to_string())),
        }
    }
}
