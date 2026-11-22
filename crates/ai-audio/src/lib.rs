use tinycut_core_domain::{DomainResult, MediaRef, Transcript, TranscriptSegment};
use uuid::Uuid;

/// Builds the ASR `initial_prompt` / hotword hint from the creator's local Personal Glossary.
pub fn build_glossary_prompt(glossary: &[String]) -> String {
    if glossary.is_empty() {
        return String::new();
    }
    format!("Glossary terms: {}.", glossary.join(", "))
}

/// Local on-device audio inference engine (wraps `whisper-rs` + `llama-cpp-2` / `mistral.rs` + `silero-vad`).
#[derive(Debug, Default)]
pub struct LocalAudioEngine {
    pub asr_model_version: String,
    pub llm_model_version: String,
}

impl LocalAudioEngine {
    pub fn new(asr_model_version: impl Into<String>, llm_model_version: impl Into<String>) -> Self {
        Self {
            asr_model_version: asr_model_version.into(),
            llm_model_version: llm_model_version.into(),
        }
    }

    pub async fn transcribe_local(
        &self,
        media: &MediaRef,
        glossary: &[String],
    ) -> DomainResult<Transcript> {
        let _prompt = build_glossary_prompt(glossary);
        // Stub for Phase 0; Phase 1 wires `whisper-rs` + `silero-vad`.
        Ok(Transcript {
            locale: "vi-VN".into(),
            asr_model_version: self.asr_model_version.clone(),
            llm_model_version: None,
            segments: vec![TranscriptSegment {
                id: Uuid::new_v4(),
                start_ms: 0,
                end_ms: media.duration_ms.min(3_000),
                raw_asr: "stub local transcription".into(),
                llm_draft: None,
                text: "stub local transcription".into(),
                speaker: None,
                words: vec![],
            }],
        })
    }

    pub async fn fix_subtitles_local(
        &self,
        raw: &Transcript,
        _glossary: &[String],
    ) -> DomainResult<Transcript> {
        // Stub for Phase 0; Phase 1 wires local quantized Qwen GGUF post-processor.
        let mut out = raw.clone();
        out.llm_model_version = Some(self.llm_model_version.clone());
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glossary_prompt_formats_terms() {
        let terms = vec!["TinyCut".into(), "MacBook Air M4".into(), "Cursor".into()];
        assert_eq!(
            build_glossary_prompt(&terms),
            "Glossary terms: TinyCut, MacBook Air M4, Cursor."
        );
    }
}
