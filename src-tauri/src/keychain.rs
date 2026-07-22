use serde::{Deserialize, Serialize};

use crate::error::Result;

/// The keychain service name every katto credential lives under.
const SERVICE: &str = "katto";

/// The credentials katto stores. Wire values are the snake_case names the
/// frontend sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum KeyService {
    Elevenlabs,
    Anthropic,
}

impl KeyService {
    /// The keychain account name for this credential.
    pub fn account(self) -> &'static str {
        match self {
            KeyService::Elevenlabs => "elevenlabs",
            KeyService::Anthropic => "anthropic",
        }
    }
}

/// Install the macOS login-keychain store as the process-wide keyring backend.
/// Call once at startup, before any entry operation. Non-macOS builds (CI) are
/// a no-op; entry operations there fail with `NoDefaultStore`, which no test
/// path reaches (tests install the mock store).
pub fn init() -> Result<()> {
    #[cfg(target_os = "macos")]
    keyring_core::set_default_store(
        apple_native_keyring_store::keychain::Store::new()
            .map_err(|e| crate::error::Error::Keychain(e.to_string()))?,
    );
    Ok(())
}

fn entry(service: KeyService) -> Result<keyring_core::Entry> {
    Ok(keyring_core::Entry::new(SERVICE, service.account())?)
}

/// Write `value` for `service`. The value is never logged and never read back
/// out to the frontend.
pub fn store_key(service: KeyService, value: &str) -> Result<()> {
    entry(service)?.set_password(value)?;
    Ok(())
}

/// Read a stored key; `Ok(None)` when absent. The value stays in the backend
/// (engine clients take it as an argument) — never log it, never cross IPC.
pub fn read_key(service: KeyService) -> Result<Option<String>> {
    match entry(service)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring_core::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Whether a credential exists for `service`, without exposing its value.
pub fn key_present(service: KeyService) -> Result<bool> {
    match entry(service)?.get_password() {
        Ok(_) => Ok(true),
        Err(keyring_core::Error::NoEntry) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `set_default_store` is process-global, so a single test owns the whole
    /// mock-store lifecycle (parallel tests would race the global).
    #[test]
    fn store_then_present_round_trips() {
        keyring_core::set_default_store(keyring_core::mock::Store::new().unwrap());

        assert!(!key_present(KeyService::Elevenlabs).unwrap());
        store_key(KeyService::Elevenlabs, "xi-test-value").unwrap();
        assert!(key_present(KeyService::Elevenlabs).unwrap());
        assert!(!key_present(KeyService::Anthropic).unwrap());
    }
}
