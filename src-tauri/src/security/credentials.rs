//! Thin wrapper over the OS credential store (Windows Credential Manager /
//! macOS Keychain / Linux Secret Service) via the `keyring` crate.
//!
//! Secrets are keyed by `(SERVICE, "<conn_id>:<field>")` so each connection's
//! credentials are isolated and can be cleaned up independently. The plaintext
//! password / passphrase never touches the on-disk JSON config — only this
//! store holds them, and they are encrypted by the OS under the current user.
//!
//! # How a secret is addressed
//!
//! Each secret is stored via `Entry::new(SERVICE, account)` where:
//! - `SERVICE`  = `"com.termcraft.app"` (matches the app identifier in
//!   `tauri.conf.json`, so all TermCraft secrets share one namespace).
//! - `account`  = `"<conn_id>:<field>"` (the connection UUID plus a field
//!   suffix, e.g. `<conn_id>:password`). The `field` distinguishes multiple
//!   secrets belonging to one connection (password vs passphrase).
//!
//! `keyring` maps that `(service, account)` pair to the platform store. The
//! exact on-disk identifier is backend-specific and not something our code
//! builds itself — but for reference:
//!
//! - **Windows**: the credential is a Generic Credential whose target name is
//!   the concatenation `account.service` — i.e.
//!   `<conn_id>:<field>.com.termcraft.app`. That is the string that shows up
//!   in Windows Credential Manager (`control /name Microsoft.CredentialManager`
//!   → Windows 凭据), and it is also what `cmdkey /list:com.termcraft.app`
//!   prints. The `service` half is the "application" attribution, the
//!   `<conn_id>:<field>` half is the account.
//! - **macOS / Linux**: the same `(service, account)` pair indexes a Keychain
//!   generic item / a Secret Service entry respectively.
//!
//! Because lookups reuse the identical `(SERVICE, account)` pair, a stored
//! secret is always found back by the same `conn_id` + `field` that wrote it.

use keyring::Entry;

use crate::errors::AppError;

/// Service name shared across all TermCraft secrets. Matches the app
/// identifier in `tauri.conf.json`. All keyring entries are namespaced under
/// it, so deleting TermCraft's entries never touches another app's.
const SERVICE: &str = "com.termcraft.app";

/// Field suffixes used to namespace a connection's secrets.
pub const PASSWORD: &str = "password";
pub const PASSPHRASE: &str = "passphrase";

/// Build the keyring `account` string for one of a connection's secrets.
///
/// One entry per `(conn_id, field)`, so a single connection may own several
/// secrets (e.g. both `password` and `passphrase`).
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
