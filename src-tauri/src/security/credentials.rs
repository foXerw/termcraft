//! Thin wrapper over the OS credential store (Windows Credential Manager /
//! macOS Keychain / Linux Secret Service) via the `keyring` crate.
//!
//! Secrets are keyed by `(SERVICE, "<conn_id>:<field>")` so each connection's
//! credentials are isolated and can be cleaned up independently. The plaintext
//! password / passphrase never touches the on-disk JSON config — only this
//! store holds them, and they are encrypted by the OS under the current user.

use keyring::Entry;

use crate::errors::AppError;

/// Service name shared across all TermCraft secrets. Matches the app
/// identifier in `tauri.conf.json`.
const SERVICE: &str = "com.termcraft.app";

/// Field suffixes used to namespace a connection's secrets.
pub const PASSWORD: &str = "password";
pub const PASSPHRASE: &str = "passphrase";

fn account(conn_id: &str, field: &str) -> String {
    format!("{conn_id}:{field}")
}

fn entry(conn_id: &str, field: &str) -> Result<Entry, AppError> {
    Entry::new(SERVICE, &account(conn_id, field))
        .map_err(|e| AppError::Config(format!("keyring entry: {e}")))
}

/// Persist `value` into the OS credential store for `(conn_id, field)`.
/// Empty values are a no-op (nothing stored) so callers can pass empty
/// markers without polluting the store.
pub fn set_secret(conn_id: &str, field: &str, value: &str) -> Result<(), AppError> {
    if value.is_empty() {
        return Ok(());
    }
    entry(conn_id,field)?
        .set_password(value)
        .map_err(|e| AppError::Config(format!("keyring set: {e}")))
}

/// Read back a stored secret. Returns `Ok(None)` when nothing is stored
/// (e.g. after migration to a new machine, or for passwordless entries).
pub fn get_secret(conn_id: &str, field: &str) -> Result<Option<String>, AppError> {
    match entry(conn_id, field) {
        Ok(entry) => match entry.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::Config(format!("keyring get: {e}"))),
        },
        Err(e) => Err(e),
    }
}

/// Remove a stored secret. Idempotent: deleting a missing entry is `Ok(())`.
pub fn delete_secret(conn_id: &str, field: &str) -> Result<(), AppError> {
    match entry(conn_id, field) {
        Ok(entry) => match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::Config(format!("keyring delete: {e}"))),
        },
        Err(e) => Err(e),
    }
}
