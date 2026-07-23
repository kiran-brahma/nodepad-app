//! Local Ollama model discovery.
//!
//! Nodepad treats model identifiers as opaque strings: it discovers what the
//! local `http://localhost:11434` host reports via Ollama's native `/api/tags`
//! endpoint and lets the thinker choose one. No catalog, capability inference,
//! or credential is introduced in this slice.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

const OLLAMA_LOCAL_BASE_URL: &str = "http://localhost:11434";

/// One failure mode the UI can act on. Distinct states let the thinker know
/// whether Ollama is missing, slow, confused, or simply empty, or whether
/// the cloud host rejected, throttled, or could not find the saved key.
/// Local discovery only ever produces the first four; the rest are reserved
/// for cloud discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryFailureCode {
    Unavailable,
    Timeout,
    MalformedResponse,
    EmptyList,
    /// The bearer key was not in the keychain at the moment of the call.
    Unauthenticated,
    /// The cloud host rejected the key.
    AuthenticationFailed,
    /// The cloud host asked the thinker to slow down.
    RateLimited,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryFailure {
    pub code: DiscoveryFailureCode,
    pub message: String,
}

/// What a discovery attempt produced. The UI never receives raw HTTP details:
/// only committed model identifiers or one typed failure.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DiscoveryOutcome {
    Committed { models: Vec<String> },
    Failed { failure: DiscoveryFailure },
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

/// The HTTP seam behind discovery. Production uses `reqwest`; tests supply a
/// fake that returns controlled response bodies without touching the network.
#[async_trait]
pub trait TagsClient: Send + Sync {
    async fn fetch_tags(&self, base_url: &str) -> Result<String, DiscoveryFailureCode>;
}

/// A production client that talks to the fixed local Ollama host.
pub struct HttpTagsClient {
    client: reqwest::Client,
}

impl HttpTagsClient {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl TagsClient for HttpTagsClient {
    async fn fetch_tags(&self, base_url: &str) -> Result<String, DiscoveryFailureCode> {
        let url = format!("{base_url}/api/tags");
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|error| if error.is_timeout() {
                DiscoveryFailureCode::Timeout
            } else {
                DiscoveryFailureCode::Unavailable
            })?;
        response
            .text()
            .await
            .map_err(|_| DiscoveryFailureCode::Unavailable)
    }
}

/// State-free provider: ask the seam for `/api/tags` and turn the response into
/// a sorted, deduplicated list of opaque model names.
pub struct OllamaProvider {
    client: Arc<dyn TagsClient>,
}

impl OllamaProvider {
    pub fn new(client: Arc<dyn TagsClient>) -> Self {
        Self { client }
    }

    pub async fn discover_models(&self) -> DiscoveryOutcome {
        match self.client.fetch_tags(OLLAMA_LOCAL_BASE_URL).await {
            Ok(body) => match parse_tags_response(&body) {
                Ok(models) => DiscoveryOutcome::Committed { models },
                Err(code) => DiscoveryOutcome::Failed {
                    failure: failure_from_code(code),
                },
            },
            Err(code) => DiscoveryOutcome::Failed {
                failure: failure_from_code(code),
            },
        }
    }
}

fn parse_tags_response(body: &str) -> Result<Vec<String>, DiscoveryFailureCode> {
    let parsed: TagsResponse =
        serde_json::from_str(body).map_err(|_| DiscoveryFailureCode::MalformedResponse)?;
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
        return Err(DiscoveryFailureCode::EmptyList);
    }
    Ok(names)
}

fn failure_from_code(code: DiscoveryFailureCode) -> DiscoveryFailure {
    let message = match code {
        DiscoveryFailureCode::Unavailable => {
            "The local Ollama host is not reachable at http://localhost:11434."
        }
        DiscoveryFailureCode::Timeout => "The local Ollama host did not respond in time.",
        DiscoveryFailureCode::MalformedResponse => {
            "The model list from Ollama was not the expected shape."
        }
        DiscoveryFailureCode::EmptyList => {
            "Ollama reported no models. Pull one with `ollama pull <model>`."
        }
        DiscoveryFailureCode::Unauthenticated => {
            "Add your Ollama Cloud key to enable Cloud AI."
        }
        DiscoveryFailureCode::AuthenticationFailed => {
            "Ollama Cloud rejected the key. Update it in Settings."
        }
        DiscoveryFailureCode::RateLimited => {
            "Ollama Cloud is throttling requests. Try again in a moment."
        }
    }
    .to_owned();
    DiscoveryFailure { code, message }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeTagsClient {
        result: Result<String, DiscoveryFailureCode>,
    }

    #[async_trait]
    impl TagsClient for FakeTagsClient {
        async fn fetch_tags(&self, _base_url: &str) -> Result<String, DiscoveryFailureCode> {
            self.result.clone()
        }
    }

    fn provider_with(result: Result<String, DiscoveryFailureCode>) -> OllamaProvider {
        OllamaProvider::new(Arc::new(FakeTagsClient { result }))
    }

    #[tokio::test]
    async fn discovers_sorted_model_names() {
        let body = r#"{"models":[{"name":"phi3:latest","model":"phi3:latest"},{"name":"llama3.1:latest","model":"llama3.1:latest"}]}"#;
        let outcome = provider_with(Ok(body.into())).discover_models().await;
        assert_eq!(
            outcome,
            DiscoveryOutcome::Committed {
                models: vec!["llama3.1:latest".into(), "phi3:latest".into()],
            }
        );
    }

    #[tokio::test]
    async fn accepts_unicode_and_opaque_identifiers() {
        let body = r#"{"models":[{"name":"先生-7b:latest"},{"name":"some/vendor/model:tag"}]}"#;
        let outcome = provider_with(Ok(body.into())).discover_models().await;
        assert_eq!(
            outcome,
            DiscoveryOutcome::Committed {
                models: vec![
                    "some/vendor/model:tag".into(),
                    "先生-7b:latest".into(),
                ],
            }
        );
    }

    #[tokio::test]
    async fn falls_back_to_model_field_when_name_missing() {
        let body = r#"{"models":[{"model":"fallback:latest"}]}"#;
        let outcome = provider_with(Ok(body.into())).discover_models().await;
        assert_eq!(
            outcome,
            DiscoveryOutcome::Committed {
                models: vec!["fallback:latest".into()],
            }
        );
    }

    #[tokio::test]
    async fn reports_unavailable_host() {
        let outcome = provider_with(Err(DiscoveryFailureCode::Unavailable))
            .discover_models()
            .await;
        assert!(
            matches!(outcome, DiscoveryOutcome::Failed { failure } if failure.code == DiscoveryFailureCode::Unavailable)
        );
    }

    #[tokio::test]
    async fn reports_timeout() {
        let outcome = provider_with(Err(DiscoveryFailureCode::Timeout))
            .discover_models()
            .await;
        assert!(
            matches!(outcome, DiscoveryOutcome::Failed { failure } if failure.code == DiscoveryFailureCode::Timeout)
        );
    }

    #[tokio::test]
    async fn reports_malformed_response() {
        let outcome = provider_with(Ok("not json".into())).discover_models().await;
        assert!(
            matches!(outcome, DiscoveryOutcome::Failed { failure } if failure.code == DiscoveryFailureCode::MalformedResponse)
        );
    }

    #[tokio::test]
    async fn reports_missing_models_field() {
        let body = r#"{"unexpected":[]}"#;
        let outcome = provider_with(Ok(body.into())).discover_models().await;
        assert!(
            matches!(outcome, DiscoveryOutcome::Failed { failure } if failure.code == DiscoveryFailureCode::MalformedResponse)
        );
    }

    #[tokio::test]
    async fn reports_empty_list() {
        let body = r#"{"models":[]}"#;
        let outcome = provider_with(Ok(body.into())).discover_models().await;
        assert!(
            matches!(outcome, DiscoveryOutcome::Failed { failure } if failure.code == DiscoveryFailureCode::EmptyList)
        );
    }

    #[tokio::test]
    async fn reports_empty_after_dropping_blank_entries() {
        let body = r#"{"models":[{"name":""},{"name":null}]}"#;
        let outcome = provider_with(Ok(body.into())).discover_models().await;
        assert!(
            matches!(outcome, DiscoveryOutcome::Failed { failure } if failure.code == DiscoveryFailureCode::EmptyList)
        );
    }
}
