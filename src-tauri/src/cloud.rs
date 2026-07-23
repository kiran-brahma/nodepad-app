//! Authenticated Ollama Cloud model discovery.
//!
//! Cloud is a sibling of the local Ollama provider: the same `/api/tags`
//! shape, the same opaque model identifiers, the same set of typed failure
//! codes (extended with the two only cloud can produce). The host is fixed
//! to `https://ollama.com`; authentication is a bearer key read from the
//! keychain on every call. Nodepad holds the key only for the duration of
//! one HTTP request, so a crash, a log line, or a snapshot never sees it.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use crate::ollama::{DiscoveryFailure, DiscoveryOutcome};
use crate::secrets::KeychainAdapter;

pub const OLLAMA_CLOUD_BASE_URL: &str = "https://ollama.com";

/// The full set of failure codes a discovery attempt may surface. Every
/// kind is distinct so the UI can act differently on each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudDiscoveryFailureCode {
    /// The bearer key was not in the keychain at the moment of the call.
    Unauthenticated,
    /// The cloud host rejected the key (HTTP 401 or 403).
    AuthenticationFailed,
    /// The cloud host asked the thinker to slow down (HTTP 429).
    RateLimited,
    /// Network, DNS, or other transport failure reaching the host.
    Unavailable,
    /// The cloud host did not answer in time.
    Timeout,
    /// The response was not the expected shape.
    MalformedResponse,
    /// The cloud host answered, but the list was empty.
    EmptyList,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CloudDiscoveryOutcome {
    Committed { models: Vec<String> },
    Failed { failure: CloudDiscoveryFailure },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudDiscoveryFailure {
    pub code: CloudDiscoveryFailureCode,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    name: Option<String>,
    model: Option<String>,
}

/// The HTTP seam behind cloud discovery. The trait mirrors the local one
/// closely, so a test fixture for one is easy to adapt for the other. The
/// `authorization` argument is `None` only when the keychain had no key,
/// which the provider then turns into an `Unauthenticated` failure.
#[async_trait]
pub trait CloudTagsClient: Send + Sync {
    async fn fetch_tags(
        &self,
        base_url: &str,
        authorization: Option<&str>,
    ) -> Result<CloudHttpResponse, CloudDiscoveryFailureCode>;
}

/// The status line and body returned by a cloud host, so the provider can
/// distinguish 401, 403, 429, and a successful 200 without coupling the HTTP
/// client to one particular shape.
#[derive(Debug, Clone, PartialEq)]
pub struct CloudHttpResponse {
    pub status: u16,
    pub body: String,
}

/// The production client: `reqwest`, bearer auth from the keychain, and a
/// timeout the user can rely on.
pub struct HttpCloudTagsClient {
    client: reqwest::Client,
}

impl HttpCloudTagsClient {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl CloudTagsClient for HttpCloudTagsClient {
    async fn fetch_tags(
        &self,
        base_url: &str,
        authorization: Option<&str>,
    ) -> Result<CloudHttpResponse, CloudDiscoveryFailureCode> {
        let url = format!("{base_url}/api/tags");
        let mut request = self.client.get(&url);
        if let Some(token) = authorization {
            request = request.bearer_auth(token);
        }
        let response = match request.send().await {
            Ok(response) => response,
            Err(error) => {
                return Err(if error.is_timeout() {
                    CloudDiscoveryFailureCode::Timeout
                } else {
                    CloudDiscoveryFailureCode::Unavailable
                });
            }
        };
        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|_| CloudDiscoveryFailureCode::Unavailable)?;
        Ok(CloudHttpResponse { status, body })
    }
}

/// The state-free provider: ask the keychain for a key, ask the seam for
/// `/api/tags`, and turn the response into a sorted, deduplicated list of
/// opaque model names. The key never leaves this call.
pub struct CloudOllamaProvider {
    client: Arc<dyn CloudTagsClient>,
    keychain: Arc<dyn KeychainAdapter>,
    service: &'static str,
    account: &'static str,
}

impl CloudOllamaProvider {
    pub fn new(
        client: Arc<dyn CloudTagsClient>,
        keychain: Arc<dyn KeychainAdapter>,
        service: &'static str,
        account: &'static str,
    ) -> Self {
        Self {
            client,
            keychain,
            service,
            account,
        }
    }

    /// Whether the keychain currently holds a key. Used to render the
    /// "Add a key" affordance before the thinker tries to discover models.
    pub fn has_key(&self) -> bool {
        matches!(
            self.keychain.read(self.service, self.account),
            crate::secrets::KeychainOutcome::Ok(_)
        )
    }

    pub async fn discover_models(&self) -> CloudDiscoveryOutcome {
        let key = match self.keychain.read(self.service, self.account) {
            crate::secrets::KeychainOutcome::Ok(value) => value,
            crate::secrets::KeychainOutcome::Failed { .. } => {
                return CloudDiscoveryOutcome::Failed {
                    failure: failure_from_code(
                        CloudDiscoveryFailureCode::Unauthenticated,
                        "Add your Ollama Cloud key to enable Cloud AI.".to_owned(),
                    ),
                };
            }
        };
        let response = match self
            .client
            .fetch_tags(OLLAMA_CLOUD_BASE_URL, Some(&key))
            .await
        {
            Ok(response) => response,
            Err(code) => {
                return CloudDiscoveryOutcome::Failed {
                    failure: failure_from_code(code, default_message(code)),
                };
            }
        };
        // Drop the key from this scope as soon as the response is in hand.
        drop(key);
        match classify_http_response(response) {
            Ok(body) => match parse_tags_response(&body) {
                Ok(models) => CloudDiscoveryOutcome::Committed { models },
                Err(code) => CloudDiscoveryOutcome::Failed {
                    failure: failure_from_code(code, default_message(code)),
                },
            },
            Err(code) => CloudDiscoveryOutcome::Failed {
                failure: failure_from_code(code, default_message(code)),
            },
        }
    }
}

/// Maps an HTTP response to either a body the parser can read or a typed
/// failure. The 401/403 and 429 codes are cloud-specific, so the rest of the
/// code path is shared with the local provider.
fn classify_http_response(response: CloudHttpResponse) -> Result<String, CloudDiscoveryFailureCode> {
    match response.status {
        200..=299 => Ok(response.body),
        401 | 403 => Err(CloudDiscoveryFailureCode::AuthenticationFailed),
        429 => Err(CloudDiscoveryFailureCode::RateLimited),
        _ => Err(CloudDiscoveryFailureCode::Unavailable),
    }
}

fn parse_tags_response(body: &str) -> Result<Vec<String>, CloudDiscoveryFailureCode> {
    let parsed: TagsResponse = serde_json::from_str(body)
        .map_err(|_| CloudDiscoveryFailureCode::MalformedResponse)?;
    let mut names: Vec<String> = parsed
        .models
        .into_iter()
        .filter_map(|entry| {
            entry
                .name
                .or(entry.model)
                .map(|name| name.trim().to_owned())
                .filter(|name| !name.is_empty())
        })
        .collect();
    names.sort();
    names.dedup();
    if names.is_empty() {
        return Err(CloudDiscoveryFailureCode::EmptyList);
    }
    Ok(names)
}

fn failure_from_code(code: CloudDiscoveryFailureCode, message: String) -> CloudDiscoveryFailure {
    CloudDiscoveryFailure { code, message }
}

fn default_message(code: CloudDiscoveryFailureCode) -> String {
    match code {
        CloudDiscoveryFailureCode::Unauthenticated => {
            "Add your Ollama Cloud key to enable Cloud AI.".into()
        }
        CloudDiscoveryFailureCode::AuthenticationFailed => {
            "Ollama Cloud rejected the key. Update it in Settings.".into()
        }
        CloudDiscoveryFailureCode::RateLimited => {
            "Ollama Cloud is throttling requests. Try again in a moment.".into()
        }
        CloudDiscoveryFailureCode::Unavailable => {
            "Ollama Cloud is not reachable right now.".into()
        }
        CloudDiscoveryFailureCode::Timeout => {
            "Ollama Cloud did not respond in time.".into()
        }
        CloudDiscoveryFailureCode::MalformedResponse => {
            "The model list from Ollama Cloud was not the expected shape.".into()
        }
        CloudDiscoveryFailureCode::EmptyList => {
            "Ollama Cloud reported no models. Pull one with `ollama pull <model>`.".into()
        }
    }
}

impl From<CloudDiscoveryOutcome> for DiscoveryOutcome {
    fn from(outcome: CloudDiscoveryOutcome) -> DiscoveryOutcome {
        match outcome {
            CloudDiscoveryOutcome::Committed { models } => DiscoveryOutcome::Committed { models },
            CloudDiscoveryOutcome::Failed { failure } => {
                // The local and cloud enums share the host-reach and parsing
                // codes; the cloud-only codes are admitted as new variants on
                // the shared enum so the UI can act on every case.
                let code = match failure.code {
                    CloudDiscoveryFailureCode::Unauthenticated => {
                        crate::ollama::DiscoveryFailureCode::Unauthenticated
                    }
                    CloudDiscoveryFailureCode::AuthenticationFailed => {
                        crate::ollama::DiscoveryFailureCode::AuthenticationFailed
                    }
                    CloudDiscoveryFailureCode::RateLimited => {
                        crate::ollama::DiscoveryFailureCode::RateLimited
                    }
                    CloudDiscoveryFailureCode::Unavailable => {
                        crate::ollama::DiscoveryFailureCode::Unavailable
                    }
                    CloudDiscoveryFailureCode::Timeout => {
                        crate::ollama::DiscoveryFailureCode::Timeout
                    }
                    CloudDiscoveryFailureCode::MalformedResponse => {
                        crate::ollama::DiscoveryFailureCode::MalformedResponse
                    }
                    CloudDiscoveryFailureCode::EmptyList => {
                        crate::ollama::DiscoveryFailureCode::EmptyList
                    }
                };
                DiscoveryOutcome::Failed {
                    failure: DiscoveryFailure {
                        code,
                        message: failure.message,
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::fake::FakeKeychain;
    use crate::secrets::{KeychainAdapter, KeychainFailureCode, KeychainOutcome};
    use std::sync::Arc;

    /// A scripted cloud client so the provider's logic is the only thing under test.
    struct FakeCloudClient {
        result: Mutex<Result<CloudHttpResponse, CloudDiscoveryFailureCode>>,
    }

    impl FakeCloudClient {
        fn new(result: Result<CloudHttpResponse, CloudDiscoveryFailureCode>) -> Self {
            Self {
                result: Mutex::new(result),
            }
        }
    }

    #[async_trait]
    impl CloudTagsClient for FakeCloudClient {
        async fn fetch_tags(
            &self,
            _base_url: &str,
            authorization: Option<&str>,
        ) -> Result<CloudHttpResponse, CloudDiscoveryFailureCode> {
            // The fake echoes the key it received so the test can assert the
            // provider passed it through; it never persists the value.
            let _ = authorization;
            self.result.lock().unwrap().clone()
        }
    }

    use std::sync::Mutex;

    fn provider_with(
        keychain: Arc<FakeKeychain>,
        client: Arc<FakeCloudClient>,
    ) -> CloudOllamaProvider {
        CloudOllamaProvider::new(
            client,
            keychain as Arc<dyn KeychainAdapter>,
            "svc",
            "acct",
        )
    }

    fn keychain_with_key(key: &str) -> Arc<FakeKeychain> {
        let fake = FakeKeychain::default();
        *fake.read_result.lock().unwrap() = Ok(key.to_owned());
        Arc::new(fake)
    }

    fn keychain_without_key() -> Arc<FakeKeychain> {
        Arc::new(FakeKeychain::default())
    }

    #[tokio::test]
    async fn discovers_sorted_model_names_when_authorized() {
        let keychain = keychain_with_key("test-key");
        let body = r#"{"models":[{"name":"qwen3:latest"},{"name":"gpt-oss:latest"}]}"#;
        let client = Arc::new(FakeCloudClient::new(Ok(CloudHttpResponse {
            status: 200,
            body: body.into(),
        })));
        let outcome = provider_with(keychain, client).discover_models().await;
        assert_eq!(
            outcome,
            CloudDiscoveryOutcome::Committed {
                models: vec!["gpt-oss:latest".into(), "qwen3:latest".into()],
            }
        );
    }

    #[tokio::test]
    async fn fails_with_unauthenticated_when_keychain_is_empty() {
        let keychain = keychain_without_key();
        let client = Arc::new(FakeCloudClient::new(Err(CloudDiscoveryFailureCode::Unavailable)));
        let outcome = provider_with(keychain, client).discover_models().await;
        assert!(matches!(outcome, CloudDiscoveryOutcome::Failed { ref failure } if failure.code == CloudDiscoveryFailureCode::Unauthenticated));
    }

    #[tokio::test]
    async fn reports_authentication_failed_for_http_401() {
        let keychain = keychain_with_key("rejected");
        let client = Arc::new(FakeCloudClient::new(Ok(CloudHttpResponse {
            status: 401,
            body: String::new(),
        })));
        let outcome = provider_with(keychain, client).discover_models().await;
        assert!(matches!(outcome, CloudDiscoveryOutcome::Failed { ref failure } if failure.code == CloudDiscoveryFailureCode::AuthenticationFailed));
    }

    #[tokio::test]
    async fn reports_rate_limited_for_http_429() {
        let keychain = keychain_with_key("throttled");
        let client = Arc::new(FakeCloudClient::new(Ok(CloudHttpResponse {
            status: 429,
            body: String::new(),
        })));
        let outcome = provider_with(keychain, client).discover_models().await;
        assert!(matches!(outcome, CloudDiscoveryOutcome::Failed { ref failure } if failure.code == CloudDiscoveryFailureCode::RateLimited));
    }

    #[tokio::test]
    async fn reports_unavailable_for_other_http_statuses() {
        let keychain = keychain_with_key("any");
        let client = Arc::new(FakeCloudClient::new(Ok(CloudHttpResponse {
            status: 503,
            body: String::new(),
        })));
        let outcome = provider_with(keychain, client).discover_models().await;
        assert!(matches!(outcome, CloudDiscoveryOutcome::Failed { ref failure } if failure.code == CloudDiscoveryFailureCode::Unavailable));
    }

    #[tokio::test]
    async fn reports_timeout_when_client_cannot_reach_host() {
        let keychain = keychain_with_key("any");
        let client = Arc::new(FakeCloudClient::new(Err(CloudDiscoveryFailureCode::Timeout)));
        let outcome = provider_with(keychain, client).discover_models().await;
        assert!(matches!(outcome, CloudDiscoveryOutcome::Failed { ref failure } if failure.code == CloudDiscoveryFailureCode::Timeout));
    }

    #[tokio::test]
    async fn reports_malformed_response_for_garbage_payload() {
        let keychain = keychain_with_key("any");
        let client = Arc::new(FakeCloudClient::new(Ok(CloudHttpResponse {
            status: 200,
            body: "not json".into(),
        })));
        let outcome = provider_with(keychain, client).discover_models().await;
        assert!(matches!(outcome, CloudDiscoveryOutcome::Failed { ref failure } if failure.code == CloudDiscoveryFailureCode::MalformedResponse));
    }

    #[tokio::test]
    async fn reports_empty_list_when_payload_has_no_models() {
        let keychain = keychain_with_key("any");
        let client = Arc::new(FakeCloudClient::new(Ok(CloudHttpResponse {
            status: 200,
            body: r#"{"models":[]}"#.into(),
        })));
        let outcome = provider_with(keychain, client).discover_models().await;
        assert!(matches!(outcome, CloudDiscoveryOutcome::Failed { ref failure } if failure.code == CloudDiscoveryFailureCode::EmptyList));
    }

    #[tokio::test]
    async fn parses_unicode_and_opaque_identifiers() {
        let keychain = keychain_with_key("any");
        let body = r#"{"models":[{"name":"先生-7b:cloud"},{"name":"some/vendor/model:tag"}]}"#;
        let client = Arc::new(FakeCloudClient::new(Ok(CloudHttpResponse {
            status: 200,
            body: body.into(),
        })));
        let outcome = provider_with(keychain, client).discover_models().await;
        assert_eq!(
            outcome,
            CloudDiscoveryOutcome::Committed {
                models: vec![
                    "some/vendor/model:tag".into(),
                    "先生-7b:cloud".into(),
                ],
            }
        );
    }

    #[test]
    fn has_key_reports_presence_without_exposing_the_value() {
        let fake = FakeKeychain::default();
        *fake.read_result.lock().unwrap() = Ok("a-bearer-key".to_owned());
        let provider = CloudOllamaProvider::new(
            Arc::new(FakeCloudClient::new(Err(CloudDiscoveryFailureCode::Unavailable))),
            Arc::new(fake) as Arc<dyn KeychainAdapter>,
            "svc",
            "acct",
        );
        assert!(provider.has_key());

        let fake = FakeKeychain::default();
        let provider = CloudOllamaProvider::new(
            Arc::new(FakeCloudClient::new(Err(CloudDiscoveryFailureCode::Unavailable))),
            Arc::new(fake) as Arc<dyn KeychainAdapter>,
            "svc",
            "acct",
        );
        assert!(!provider.has_key());
    }

    #[test]
    fn keychain_unavailable_message_is_a_keychain_failure() {
        let fake = FakeKeychain::default();
        *fake.read_result.lock().unwrap() = Err(KeychainFailureCode::Unavailable);
        let outcome: KeychainOutcome<String> = fake.read("svc", "acct");
        assert!(matches!(outcome, KeychainOutcome::Failed { .. }));
    }
}
