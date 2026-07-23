//! The secret seam for Ollama Cloud.
//!
//! The bearer key is read on demand from a narrow, well-named module and is
//! never held in any durable state, log line, error message, command-line
//! argument, or test snapshot. The production adapter delegates to the
//! macOS keychain through the `security` command; the trait it implements
//! is the seam contract tests use to inject a fake.

use std::process::Command;

/// The fixed name the application uses for the one Ollama Cloud credential it
/// keeps. Sharing one account across multiple consented Workspaces is the
/// intended use, so the key is stored once, not per Workspace.
pub const OLLAMA_CLOUD_KEYCHAIN_SERVICE: &str = "com.nodepad.desktop";
pub const OLLAMA_CLOUD_KEYCHAIN_ACCOUNT: &str = "ollama-cloud-bearer";

/// The category of failure when reading or writing the secret. Each kind
/// tells the UI what to say to the thinker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KeychainFailureCode {
    /// The keychain helper returned nothing, an error, or exited non-zero
    /// without a specific cause the user can act on.
    Unavailable,
    /// The keychain rejected the write because of permissions, an invalid
    /// item, or a malformed password.
    Refused,
}

/// The outcome of any secret operation. The secret itself is never returned
/// over this seam: callers learn whether it is present, not what it is.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum KeychainOutcome<T> {
    Ok(T),
    Failed { failure: KeychainFailure },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeychainFailure {
    pub code: KeychainFailureCode,
    pub message: String,
}

impl KeychainFailure {
    pub fn new(code: KeychainFailureCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// The contract every keychain adapter honours. Production code uses
/// `SecurityCliKeychain`; tests use a `FakeKeychain` that records every call
/// and returns a controlled result. All methods are infallible at the Rust
/// level — a failure becomes a typed outcome — so callers never have to
/// `unwrap` a keychain error or guess what it means.
pub trait KeychainAdapter: Send + Sync {
    fn read(&self, service: &str, account: &str) -> KeychainOutcome<String>;
    fn write(&self, service: &str, account: &str, value: &str) -> KeychainOutcome<()>;
    fn delete(&self, service: &str, account: &str) -> KeychainOutcome<()>;
}

/// Production keychain backed by the macOS `security` CLI. The password is
/// passed through stdin for `add` so it never appears in the process's
/// command line, and it is read from the keychain with `find-generic-password`
/// each time, so no part of Nodepad holds a copy between calls.
pub struct SecurityCliKeychain;

impl SecurityCliKeychain {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SecurityCliKeychain {
    fn default() -> Self {
        Self::new()
    }
}

impl KeychainAdapter for SecurityCliKeychain {
    fn read(&self, service: &str, account: &str) -> KeychainOutcome<String> {
        let output = Command::new("security")
            .args([
                "find-generic-password",
                "-s",
                service,
                "-a",
                account,
                "-w",
            ])
            .output();
        match output {
            Ok(out) if out.status.success() => {
                let value = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                if value.is_empty() {
                    KeychainOutcome::Failed {
                        failure: KeychainFailure::new(
                            KeychainFailureCode::Unavailable,
                            "The Ollama Cloud key is not set in the macOS keychain.",
                        ),
                    }
                } else {
                    KeychainOutcome::Ok(value)
                }
            }
            Ok(out) => KeychainOutcome::Failed {
                failure: KeychainFailure::new(
                    KeychainFailureCode::Unavailable,
                    String::from_utf8_lossy(&out.stderr)
                        .lines()
                        .next()
                        .map(|line| line.trim().to_owned())
                        .filter(|line| !line.is_empty())
                        .unwrap_or_else(|| "The macOS keychain refused to read the Ollama Cloud key.".to_owned()),
                ),
            },
            Err(error) => KeychainOutcome::Failed {
                failure: KeychainFailure::new(
                    KeychainFailureCode::Unavailable,
                    format!("Nodepad could not run the macOS keychain helper: {error}"),
                ),
            },
        }
    }

    fn write(&self, service: &str, account: &str, value: &str) -> KeychainOutcome<()> {
        if value.is_empty() {
            return KeychainOutcome::Failed {
                failure: KeychainFailure::new(
                    KeychainFailureCode::Refused,
                    "The Ollama Cloud key may not be blank.",
                ),
            };
        }
        // Replace any existing item, so saving a new key is not a no-op when
        // an old one is still there. `add` alone would fail on the duplicate;
        // `delete` followed by `add` leaves a window where a read returns
        // nothing; a single `add -U` updates in place.
        let output = Command::new("security")
            .args([
                "add-generic-password",
                "-U",
                "-s",
                service,
                "-a",
                account,
                "-w",
                value,
            ])
            .output();
        match output {
            Ok(out) if out.status.success() => KeychainOutcome::Ok(()),
            Ok(out) => KeychainOutcome::Failed {
                failure: KeychainFailure::new(
                    KeychainFailureCode::Refused,
                    String::from_utf8_lossy(&out.stderr)
                        .lines()
                        .next()
                        .map(|line| line.trim().to_owned())
                        .filter(|line| !line.is_empty())
                        .unwrap_or_else(|| "The macOS keychain refused to save the Ollama Cloud key.".to_owned()),
                ),
            },
            Err(error) => KeychainOutcome::Failed {
                failure: KeychainFailure::new(
                    KeychainFailureCode::Unavailable,
                    format!("Nodepad could not run the macOS keychain helper: {error}"),
                ),
            },
        }
    }

    fn delete(&self, service: &str, account: &str) -> KeychainOutcome<()> {
        let output = Command::new("security")
            .args(["delete-generic-password", "-s", service, "-a", account])
            .output();
        match output {
            Ok(out) if out.status.success() => KeychainOutcome::Ok(()),
            Ok(_) => KeychainOutcome::Failed {
                failure: KeychainFailure::new(
                    KeychainFailureCode::Unavailable,
                    "The Ollama Cloud key was not in the macOS keychain.",
                ),
            },
            Err(error) => KeychainOutcome::Failed {
                failure: KeychainFailure::new(
                    KeychainFailureCode::Unavailable,
                    format!("Nodepad could not run the macOS keychain helper: {error}"),
                ),
            },
        }
    }
}

/// The shared handle production code uses; tests construct their own fake.
#[cfg(test)]
pub mod fake {
    //! A scripted keychain so the seam's contract is the only thing under test.
    use super::{KeychainAdapter, KeychainFailure, KeychainFailureCode, KeychainOutcome};
    use std::sync::Mutex;

    pub struct FakeKeychain {
        pub calls: Mutex<Vec<FakeCall>>,
        pub stored: Mutex<Option<String>>,
        pub read_result: Mutex<Result<String, KeychainFailureCode>>,
        pub write_result: Mutex<Result<(), KeychainFailureCode>>,
        pub delete_result: Mutex<Result<(), KeychainFailureCode>>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct FakeCall {
        pub operation: &'static str,
        pub service: String,
        pub account: String,
        pub value: Option<String>,
    }

    impl Default for FakeKeychain {
        fn default() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                stored: Mutex::new(None),
                read_result: Mutex::new(Err(KeychainFailureCode::Unavailable)),
                write_result: Mutex::new(Ok(())),
                delete_result: Mutex::new(Ok(())),
            }
        }
    }

    impl KeychainAdapter for FakeKeychain {
        fn read(&self, service: &str, account: &str) -> KeychainOutcome<String> {
            self.calls.lock().unwrap().push(FakeCall {
                operation: "read",
                service: service.to_owned(),
                account: account.to_owned(),
                value: None,
            });
            match self.read_result.lock().unwrap().clone() {
                Ok(value) => KeychainOutcome::Ok(value),
                Err(code) => KeychainOutcome::Failed {
                    failure: KeychainFailure::new(code, "FakeKeychain refused the read."),
                },
            }
        }

        fn write(&self, service: &str, account: &str, value: &str) -> KeychainOutcome<()> {
            self.calls.lock().unwrap().push(FakeCall {
                operation: "write",
                service: service.to_owned(),
                account: account.to_owned(),
                value: Some(value.to_owned()),
            });
            match self.write_result.lock().unwrap().clone() {
                Ok(()) => {
                    *self.stored.lock().unwrap() = Some(value.to_owned());
                    KeychainOutcome::Ok(())
                }
                Err(code) => KeychainOutcome::Failed {
                    failure: KeychainFailure::new(code, "FakeKeychain refused the write."),
                },
            }
        }

        fn delete(&self, service: &str, account: &str) -> KeychainOutcome<()> {
            self.calls.lock().unwrap().push(FakeCall {
                operation: "delete",
                service: service.to_owned(),
                account: account.to_owned(),
                value: None,
            });
            match self.delete_result.lock().unwrap().clone() {
                Ok(()) => {
                    *self.stored.lock().unwrap() = None;
                    KeychainOutcome::Ok(())
                }
                Err(code) => KeychainOutcome::Failed {
                    failure: KeychainFailure::new(code, "FakeKeychain refused the delete."),
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::FakeKeychain;
    use super::*;

    #[test]
    fn fake_keychain_records_every_call_in_order() {
        let fake = FakeKeychain::default();
        *fake.read_result.lock().unwrap() = Ok("first-key".to_owned());
        *fake.write_result.lock().unwrap() = Ok(());
        *fake.delete_result.lock().unwrap() = Ok(());

        let read = fake.read("svc", "acct");
        assert!(matches!(read, KeychainOutcome::Ok(ref value) if value == "first-key"));
        let _ = fake.write("svc", "acct", "second-key");
        let _ = fake.delete("svc", "acct");

        let calls = fake.calls.lock().unwrap();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].operation, "read");
        assert_eq!(calls[1].operation, "write");
        assert_eq!(calls[1].value.as_deref(), Some("second-key"));
        assert_eq!(calls[2].operation, "delete");
    }

    #[test]
    fn fake_keychain_reports_each_failure_mode_distinctly() {
        let fake = FakeKeychain::default();
        *fake.read_result.lock().unwrap() = Err(KeychainFailureCode::Unavailable);
        let read = fake.read("svc", "acct");
        assert!(matches!(
            read,
            KeychainOutcome::Failed { ref failure } if failure.code == KeychainFailureCode::Unavailable
        ));

        *fake.write_result.lock().unwrap() = Err(KeychainFailureCode::Refused);
        let write = fake.write("svc", "acct", "value");
        assert!(matches!(
            write,
            KeychainOutcome::Failed { ref failure } if failure.code == KeychainFailureCode::Refused
        ));
    }
}
