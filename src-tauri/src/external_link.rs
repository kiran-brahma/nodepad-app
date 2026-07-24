//! External links leave Nodepad through the macOS shell opener, and only
//! after their scheme is validated. The webview never navigates: the
//! frontend prevents every anchor click and asks [`open_external_link`] to
//! open the URL instead.
//!
//! V0 opens only `http` and `https`. Every other scheme — `mailto`, `tel`,
//! `file`, `javascript`, custom app schemes — is rejected so a Note can never
//! pull the thinker out of the webview or launch an unexpected handler.

use std::process::Command;

/// The only schemes V0 will open externally. This is the single place that
/// decides what may leave the app, so a future relaxation changes one
/// predicate instead of scattered checks.
///
/// `url::Url::parse` rejects bare strings and schemes that cannot be a base;
/// the scheme match then keeps exactly `http` and `https`. Userinfo
/// (`https://user:pass@host`) is not a scheme concern and stays openable —
/// the bearer key for AI lives in the keychain, never in a Note link.
pub fn is_openable_external_url(raw: &str) -> bool {
    let Ok(parsed) = url::Url::parse(raw) else {
        return false;
    };
    matches!(parsed.scheme(), "http" | "https")
}

/// The macOS shell opener outcome for one requested URL.
#[derive(serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum OpenLinkOutcome {
    /// The validated URL was handed to the macOS opener.
    Opened,
    /// The scheme was rejected; nothing was opened and the webview stayed.
    Rejected,
    /// The opener itself failed after validation.
    Failed { message: String },
}

/// Opens `url` in the thinker's default browser through the macOS `open`
/// command. The webview never handles the URL: the frontend calls this only
/// after `preventDefault`-ing the anchor click. `open` receives the URL as a
/// single argument, so it is never interpreted by a shell.
#[tauri::command]
pub fn open_external_link(url: String) -> OpenLinkOutcome {
    if !is_openable_external_url(&url) {
        return OpenLinkOutcome::Rejected;
    }
    match Command::new("open").arg(&url).status() {
        Ok(status) if status.success() => OpenLinkOutcome::Opened,
        Ok(_) => OpenLinkOutcome::Failed {
            message: "The macOS opener exited with an error.".to_owned(),
        },
        Err(error) => OpenLinkOutcome::Failed {
            message: error.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_http_and_https() {
        assert!(is_openable_external_url("http://example.com"));
        assert!(is_openable_external_url("https://example.com/a/b?c=1#frag"));
        assert!(is_openable_external_url("https://example.com:8443/"));
    }

    #[test]
    fn rejects_non_http_schemes() {
        assert!(!is_openable_external_url("mailto:hello@example.com"));
        assert!(!is_openable_external_url("tel:+15551234567"));
        assert!(!is_openable_external_url("file:///etc/passwd"));
        assert!(!is_openable_external_url("javascript:alert(1)"));
        assert!(!is_openable_external_url("slack://channel/abc"));
        assert!(!is_openable_external_url("data:text/plain,hi"));
    }

    #[test]
    fn rejects_unparseable_and_relative_urls() {
        assert!(!is_openable_external_url("not a url at all"));
        assert!(!is_openable_external_url("/local/path"));
        assert!(!is_openable_external_url(""));
        assert!(!is_openable_external_url("example.com"));
    }
}
