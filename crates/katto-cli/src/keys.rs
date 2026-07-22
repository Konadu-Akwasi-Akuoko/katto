//! API-key resolution: env var first, then the shared "katto" keychain
//! service (same constants as the app crate). Values are never logged.

/// Which credential to resolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyName {
    Elevenlabs,
    Anthropic,
}

impl KeyName {
    fn env_var(self) -> &'static str {
        match self {
            KeyName::Elevenlabs => "ELEVENLABS_API_KEY",
            KeyName::Anthropic => "ANTHROPIC_API_KEY",
        }
    }

    fn account(self) -> &'static str {
        match self {
            KeyName::Elevenlabs => "elevenlabs",
            KeyName::Anthropic => "anthropic",
        }
    }
}

/// Where a resolved key came from (or that it is absent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    Env,
    Keychain,
    Missing,
}

impl KeySource {
    /// Stable machine-readable name for JSON output.
    pub fn as_str(self) -> &'static str {
        match self {
            KeySource::Env => "env",
            KeySource::Keychain => "keychain",
            KeySource::Missing => "missing",
        }
    }
}

/// The keychain service name shared with the app crate.
const SERVICE: &str = "katto";

/// Install the macOS login-keychain store as the keyring backend. No-op off
/// macOS; failures degrade to keychain lookups reporting Missing.
pub fn init_keychain() {
    #[cfg(target_os = "macos")]
    if let Ok(store) = apple_native_keyring_store::keychain::Store::new() {
        keyring_core::set_default_store(store);
    }
}

/// Resolve a key: env var first, then keychain. The value is returned for use
/// in request headers only — callers must never log it.
pub fn resolve(name: KeyName) -> (Option<String>, KeySource) {
    if let Ok(v) = std::env::var(name.env_var())
        && !v.is_empty()
    {
        return (Some(v), KeySource::Env);
    }
    let from_keychain = keyring_core::Entry::new(SERVICE, name.account())
        .ok()
        .and_then(|e| e.get_password().ok());
    match from_keychain {
        Some(v) => (Some(v), KeySource::Keychain),
        None => (None, KeySource::Missing),
    }
}
