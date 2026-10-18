//! "Check for updates": compares the running version against the latest
//! stable GitHub release and renders a localized result for the overlay.
//!
//! The network call is a thin, isolated function; the version comparison is
//! pure so it is fully unit-testable without a network.

use serde::{Deserialize, Serialize};

use crate::config::Language;
use crate::locale;

/// Structured update status information for UI surfaces and CLI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub is_update_available: bool,
    pub release_url: Option<String>,
    pub message: String,
}

/// Pure version/status resolution from an explicit current version and latest tag result.
pub fn resolve_update_info(
    lang: Language,
    current_version: &str,
    latest: Result<String, String>,
) -> UpdateInfo {
    let s = locale::strings(lang);
    match latest {
        Ok(tag) => {
            let newer = is_newer(&tag, current_version);
            let message = if newer {
                format!("{} {tag}", s.update_check_available_prefix)
            } else {
                s.update_check_up_to_date.to_string()
            };
            let release_url = if newer {
                Some(format!(
                    "https://github.com/dt418/volume-control/releases/tag/{tag}"
                ))
            } else {
                None
            };
            UpdateInfo {
                current_version: current_version.to_string(),
                latest_version: Some(tag),
                is_update_available: newer,
                release_url,
                message,
            }
        }
        Err(error) => {
            log::warn!("{error}");
            UpdateInfo {
                current_version: current_version.to_string(),
                latest_version: None,
                is_update_available: false,
                release_url: None,
                message: s.update_check_failed.to_string(),
            }
        }
    }
}

/// Fetch the latest tag and return structured update info.
pub fn check_update_info(lang: Language) -> Result<UpdateInfo, String> {
    Ok(resolve_update_info(
        lang,
        env!("CARGO_PKG_VERSION"),
        fetch_latest_tag(),
    ))
}
/// GitHub API endpoint for the latest stable release.
const RELEASES_LATEST_API: &str =
    "https://api.github.com/repos/dt418/volume-control/releases/latest";

/// Parse `vX.Y.Z` (also accepting a bare `X.Y.Z`) into a comparable triple.
pub fn parse_version(tag: &str) -> Option<(u64, u64, u64)> {
    let trimmed = tag.trim();
    let body = trimmed.strip_prefix('v').unwrap_or(trimmed);
    let mut parts = body.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// True when `latest` is a strictly newer version than `current`. Unparsable
/// input on either side is never "newer" (fail closed: no nagging on garbage).
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

/// Fetch the latest stable release tag (`vX.Y.Z`). Blocking; ~seconds.
pub fn fetch_latest_tag() -> Result<String, String> {
    let response = ureq::get(RELEASES_LATEST_API)
        .set("Accept", "application/vnd.github+json")
        .set(
            "User-Agent",
            concat!("VolumeControl/", env!("CARGO_PKG_VERSION")),
        )
        .timeout(std::time::Duration::from_secs(10))
        .call()
        .map_err(|error| format!("update check request failed: {error}"))?;
    let body: serde_json::Value = response
        .into_json()
        .map_err(|error| format!("update check response invalid: {error}"))?;
    body.get("tag_name")
        .and_then(|value| value.as_str())
        .map(str::to_owned)
        .ok_or_else(|| "update check response missing tag_name".to_string())
}

/// Localized overlay message for the whole check (never panics, never
/// propagates the raw error to the user).
pub fn check_for_updates(lang: Language) -> String {
    check_update_info(lang)
        .map(|info| info.message)
        .unwrap_or_else(|e| e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_accepts_prefixed_and_bare_tags() {
        assert_eq!(parse_version("v0.1.5"), Some((0, 1, 5)));
        assert_eq!(parse_version("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version(" v10.20.30 "), Some((10, 20, 30)));
        assert_eq!(parse_version("v0.1"), None);
        assert_eq!(parse_version("v0.1.5.2"), None);
        assert_eq!(parse_version("vX.Y.Z"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn is_newer_compares_every_component() {
        assert!(is_newer("v0.1.5", "0.1.4"));
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(is_newer("v1.0.0", "0.9.9"));
        assert!(!is_newer("v0.1.4", "0.1.4"));
        assert!(!is_newer("v0.1.3", "0.1.4"));
        // Numeric, not lexicographic: 10 > 9.
        assert!(is_newer("v0.10.0", "0.9.0"));
    }

    #[test]
    fn is_newer_fails_closed_on_malformed_input() {
        assert!(!is_newer("nightly", "0.1.4"));
        assert!(!is_newer("v0.1.5", "dev"));
        assert!(!is_newer("", ""));
    }

    #[test]
    fn check_messages_are_localized() {
        let en = locale::strings(Language::En);
        let vi = locale::strings(Language::Vi);
        assert_ne!(en.update_check_up_to_date, vi.update_check_up_to_date);
        assert_ne!(
            en.update_check_available_prefix,
            vi.update_check_available_prefix
        );
        assert_ne!(en.update_check_failed, vi.update_check_failed);
        assert_ne!(en.update_check_in_progress, vi.update_check_in_progress);
    }
    #[test]
    fn resolve_update_info_reports_available_and_up_to_date() {
        let available = resolve_update_info(Language::En, "0.1.7", Ok("v0.1.8".into()));
        assert!(available.is_update_available);
        assert_eq!(available.latest_version.as_deref(), Some("v0.1.8"));
        assert!(available.release_url.as_deref().unwrap().contains("v0.1.8"));

        let up_to_date = resolve_update_info(Language::En, "0.1.7", Ok("v0.1.7".into()));
        assert!(!up_to_date.is_update_available);
        assert_eq!(up_to_date.latest_version.as_deref(), Some("v0.1.7"));
        assert_eq!(up_to_date.release_url, None);

        let err = resolve_update_info(Language::En, "0.1.7", Err("network error".into()));
        assert!(!err.is_update_available);
        assert_eq!(err.latest_version, None);
    }
}
