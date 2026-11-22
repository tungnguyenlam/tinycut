use async_trait::async_trait;
use tinycut_core_domain::{
    DomainError, DomainResult, InferenceBackend, MediaRef, Project, RedactionTrack, ShieldConfig,
    Transcript,
};

/// On-device PII scrubber run BEFORE any opt-in text diff leaves the device.
/// Redacts emails, URLs, API key prefixes (`sk-...`), and long numeric sequences (phones/bank/IDs).
pub fn scrub_pii(input: &str) -> String {
    input
        .split_whitespace()
        .map(scrub_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn scrub_token(token: &str) -> String {
    let trimmed = token.trim_matches(|c: char| matches!(c, ',' | '.' | ';' | ':' | ')' | '('));
    if trimmed.contains('@') && trimmed.contains('.') {
        return "[REDACTED_EMAIL]".into();
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return "[REDACTED_URL]".into();
    }
    if trimmed.starts_with("sk-") && trimmed.len() >= 12 {
        return "[REDACTED_KEY]".into();
    }
    let digit_count = trimmed.chars().filter(|c| c.is_ascii_digit()).count();
    if digit_count >= 9 {
        return "[REDACTED_NUMBER]".into();
    }
    token.to_string()
}

/// Checks both user opt-in consent and the project's NDA/Private lock.
pub fn can_send_telemetry(user_opted_in: bool, project: &Project) -> bool {
    user_opted_in && project.allow_cloud_or_telemetry().is_ok()
}

/// Paid Cloud Boost backend (Smart Proxy Offloading -> R2 -> Serverless PyTorch GPU Worker).
#[derive(Debug, Clone)]
pub struct CloudBoostBackend {
    pub api_base_url: String,
    pub auth_token: Option<String>,
}

impl CloudBoostBackend {
    pub fn new(api_base_url: impl Into<String>, auth_token: Option<String>) -> Self {
        Self {
            api_base_url: api_base_url.into(),
            auth_token,
        }
    }
}

#[async_trait]
impl InferenceBackend for CloudBoostBackend {
    async fn transcribe(&self, _media: &MediaRef, _glossary: &[String]) -> DomainResult<Transcript> {
        Err(DomainError::Inference(
            "CloudBoostBackend::transcribe is wired in Phase 2 (v0.2)".into(),
        ))
    }

    async fn fix_subtitles(
        &self,
        _raw: &Transcript,
        _glossary: &[String],
    ) -> DomainResult<Transcript> {
        Err(DomainError::Inference(
            "CloudBoostBackend::fix_subtitles is wired in Phase 2 (v0.2)".into(),
        ))
    }

    async fn detect_privacy(
        &self,
        _media: &MediaRef,
        _cfg: &ShieldConfig,
    ) -> DomainResult<Vec<RedactionTrack>> {
        Err(DomainError::Inference(
            "CloudBoostBackend::detect_privacy is wired in Phase 2 (v0.2)".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrubs_emails_phones_urls_and_keys() {
        let raw = "Liên hệ test@example.com hoặc 0912345678 xem https://secret.vn key sk-1234567890abcdef";
        let scrubbed = scrub_pii(raw);
        assert_eq!(
            scrubbed,
            "Liên hệ [REDACTED_EMAIL] hoặc [REDACTED_NUMBER] xem [REDACTED_URL] key [REDACTED_KEY]"
        );
    }
}
