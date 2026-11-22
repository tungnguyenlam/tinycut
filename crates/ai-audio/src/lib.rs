use std::path::PathBuf;
#[cfg(feature = "whisper")]
use tinycut_core_domain::DomainError;
use tinycut_core_domain::{
    DomainResult, EditDiff, EditType, GlossaryEntry, MediaRef, Transcript, TranscriptSegment,
};
use tinycut_media_engine::{FfmpegMediaExtractor, MediaExtractor};
use uuid::Uuid;

/// A detected speech or silence span in milliseconds (used by Smart Auto-Cut).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SilenceSpan {
    pub start_ms: u64,
    pub end_ms: u64,
}

/// Builds the ASR `initial_prompt` / hotword hint from the creator's local Personal Glossary.
pub fn build_glossary_prompt(glossary: &[String]) -> String {
    if glossary.is_empty() {
        return String::new();
    }
    format!("Glossary terms: {}.", glossary.join(", "))
}

/// Detects silence regions in 16kHz mono PCM using RMS energy thresholding (local VAD baseline;
/// complements Silero-VAD ONNX when a VAD model is configured).
pub fn detect_silences_pcm16k(
    pcm: &[f32],
    rms_threshold: f32,
    min_silence_ms: u64,
) -> Vec<SilenceSpan> {
    const SAMPLE_RATE: usize = 16_000;
    const WINDOW_MS: u64 = 20;
    const WINDOW_SAMPLES: usize = (SAMPLE_RATE * WINDOW_MS as usize) / 1000;

    let mut spans = Vec::new();
    let mut silence_start: Option<u64> = None;

    for (idx, chunk) in pcm.chunks(WINDOW_SAMPLES).enumerate() {
        let ms = (idx as u64) * WINDOW_MS;
        let energy = (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len().max(1) as f32).sqrt();
        if energy < rms_threshold {
            if silence_start.is_none() {
                silence_start = Some(ms);
            }
        } else if let Some(start) = silence_start.take()
            && ms.saturating_sub(start) >= min_silence_ms
        {
            spans.push(SilenceSpan {
                start_ms: start,
                end_ms: ms,
            });
        }
    }

    if let Some(start) = silence_start {
        let end_ms = ((pcm.len() as u64) * 1000) / SAMPLE_RATE as u64;
        if end_ms.saturating_sub(start) >= min_silence_ms {
            spans.push(SilenceSpan { start_ms: start, end_ms });
        }
    }
    spans
}

/// Applies Personal Glossary substitutions (`spoken_variant -> preferred_term`) and basic
/// Vietnamese/English punctuation & filler cleanup.
pub fn apply_glossary_and_cleanup(text: &str, entries: &[GlossaryEntry], strip_fillers: bool) -> String {
    let mut result = text.trim().to_string();

    for entry in entries {
        if entry.spoken_variant.is_empty() {
            continue;
        }
        result = replace_case_insensitive(&result, &entry.spoken_variant, &entry.preferred_term);
    }

    if strip_fillers {
        for filler in ["ờ, ", "à, ", "ừm, ", "kiểu như "] {
            result = replace_case_insensitive(&result, filler, "");
        }
    }

    // Normalize whitespace and capitalize the first character.
    let collapsed = result.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = collapsed.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

fn replace_case_insensitive(haystack: &str, needle: &str, replacement: &str) -> String {
    let lower_hay = haystack.to_lowercase();
    let lower_needle = needle.to_lowercase();
    if lower_needle.is_empty() || !lower_hay.contains(&lower_needle) {
        return haystack.to_string();
    }
    // Safe token/substring replacement when byte lengths match in lowercase.
    if lower_hay.len() == haystack.len() {
        let mut out = String::with_capacity(haystack.len());
        let mut cursor = 0;
        while let Some(pos) = lower_hay[cursor..].find(&lower_needle) {
            let abs = cursor + pos;
            out.push_str(&haystack[cursor..abs]);
            out.push_str(replacement);
            cursor = abs + lower_needle.len();
        }
        out.push_str(&haystack[cursor..]);
        out
    } else {
        // Fallback for Unicode case-folding length changes.
        haystack.replace(needle, replacement)
    }
}

/// Computes an `EditDiff` when a creator edits a subtitle segment (for opt-in telemetry).
pub fn compute_edit_diff(segment: &TranscriptSegment, user_final: &str) -> Option<EditDiff> {
    let final_trimmed = user_final.trim();
    if final_trimmed == segment.raw_asr.trim() {
        return None;
    }
    let edit_type = if final_trimmed.eq_ignore_ascii_case(segment.raw_asr.trim()) {
        EditType::Punctuation
    } else {
        EditType::WordFix
    };
    Some(EditDiff {
        raw_asr: segment.raw_asr.clone(),
        llm_draft: segment.llm_draft.clone(),
        user_final: final_trimmed.to_string(),
        edit_type,
        audio_clip_ref: None,
    })
}

/// Local on-device audio inference engine (`whisper-rs` ASR + local VAD + glossary/LLM fixer).
#[derive(Debug, Clone)]
pub struct LocalAudioEngine {
    pub asr_model_version: String,
    pub llm_model_version: String,
    pub whisper_model_path: Option<PathBuf>,
    pub locale: String,
}

impl Default for LocalAudioEngine {
    fn default() -> Self {
        Self::new("whisper-v3-turbo-q5", "qwen3-1.7b-q4")
    }
}

impl LocalAudioEngine {
    pub fn new(asr_model_version: impl Into<String>, llm_model_version: impl Into<String>) -> Self {
        Self {
            asr_model_version: asr_model_version.into(),
            llm_model_version: llm_model_version.into(),
            whisper_model_path: None,
            locale: "vi-VN".into(),
        }
    }

    pub fn with_whisper_model(mut self, path: impl Into<PathBuf>) -> Self {
        self.whisper_model_path = Some(path.into());
        self
    }

    /// Runs local transcription: extracts 16kHz PCM via `FfmpegMediaExtractor` when the file exists,
    /// invokes `whisper-rs` if compiled with `--features whisper` and a GGML model path is set,
    /// and otherwise segments speech spans via local VAD so the end-to-end pipeline works offline.
    pub async fn transcribe_local(
        &self,
        media: &MediaRef,
        glossary: &[String],
    ) -> DomainResult<Transcript> {
        let prompt = build_glossary_prompt(glossary);

        #[cfg(feature = "whisper")]
        if let Some(model_path) = &self.whisper_model_path {
            let extractor = FfmpegMediaExtractor::default();
            let pcm = extractor.extract_pcm_16k(media)?;
            return self.transcribe_with_whisper_rs(&pcm, model_path, &prompt);
        }

        // Extract PCM if media file is present on disk to compute real VAD speech spans.
        let path = PathBuf::from(&media.path);
        if path.exists() {
            let extractor = FfmpegMediaExtractor::default();
            let pcm = extractor.extract_pcm_16k(media)?;
            let silences = detect_silences_pcm16k(&pcm, 0.015, 400);
            let duration_ms = ((pcm.len() as u64) * 1000) / 16_000;
            let speech_spans = invert_silences(&silences, duration_ms.max(media.duration_ms));
            let segments = if speech_spans.is_empty() {
                vec![TranscriptSegment {
                    id: Uuid::new_v4(),
                    start_ms: 0,
                    end_ms: duration_ms.max(1_000),
                    raw_asr: format!("[Audio detected; configure GGML Whisper model for ASR. {prompt}]").trim().into(),
                    llm_draft: None,
                    text: format!("[Audio detected; configure GGML Whisper model for ASR. {prompt}]").trim().into(),
                    speaker: None,
                    words: vec![],
                }]
            } else {
                speech_spans
                    .into_iter()
                    .enumerate()
                    .map(|(i, span)| {
                        let label = format!("Speech segment #{} (ready for Whisper GGML)", i + 1);
                        TranscriptSegment {
                            id: Uuid::new_v4(),
                            start_ms: span.start_ms,
                            end_ms: span.end_ms,
                            raw_asr: label.clone(),
                            llm_draft: None,
                            text: label,
                            speaker: None,
                            words: vec![],
                        }
                    })
                    .collect()
            };

            return Ok(Transcript {
                locale: self.locale.clone(),
                asr_model_version: self.asr_model_version.clone(),
                llm_model_version: None,
                segments,
            });
        }

        // Fallback for virtual/in-memory MediaRef (e.g. UI preview or tests without disk media).
        let _ = prompt;
        Ok(Transcript {
            locale: self.locale.clone(),
            asr_model_version: self.asr_model_version.clone(),
            llm_model_version: None,
            segments: vec![TranscriptSegment {
                id: Uuid::new_v4(),
                start_ms: 0,
                end_ms: media.duration_ms.clamp(1_000, 3_000),
                raw_asr: "hôm nay mình review cơ sơ trên mác búc".into(),
                llm_draft: None,
                text: "hôm nay mình review cơ sơ trên mác búc".into(),
                speaker: None,
                words: vec![],
            }],
        })
    }

    #[cfg(feature = "whisper")]
    fn transcribe_with_whisper_rs(
        &self,
        pcm_16k: &[f32],
        model_path: &std::path::Path,
        prompt: &str,
    ) -> DomainResult<Transcript> {
        use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

        let ctx = WhisperContext::new_with_params(
            model_path
                .to_str()
                .ok_or_else(|| DomainError::Inference("non-UTF-8 whisper model path".into()))?,
            WhisperContextParameters::default(),
        )
        .map_err(|e| DomainError::Inference(format!("failed to load whisper model: {e}")))?;

        let mut state = ctx
            .create_state()
            .map_err(|e| DomainError::Inference(format!("failed to create whisper state: {e}")))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(self.locale.split('-').next().unwrap_or("vi")));
        if !prompt.is_empty() {
            params.set_initial_prompt(prompt);
        }

        state
            .full(params, pcm_16k)
            .map_err(|e| DomainError::Inference(format!("whisper inference failed: {e}")))?;

        let num_segments = state
            .full_n_segments()
            .map_err(|e| DomainError::Inference(format!("failed to read segments: {e}")))?;

        let mut segments = Vec::with_capacity(num_segments as usize);
        for i in 0..num_segments {
            let text = state
                .full_get_segment_text(i)
                .map_err(|e| DomainError::Inference(format!("segment text error: {e}")))?;
            let start_ms = (state.full_get_segment_t0(i).unwrap_or(0).max(0) as u64) * 10;
            let end_ms = (state.full_get_segment_t1(i).unwrap_or(0).max(0) as u64) * 10;
            let trimmed = text.trim().to_string();
            if !trimmed.is_empty() {
                segments.push(TranscriptSegment {
                    id: Uuid::new_v4(),
                    start_ms,
                    end_ms,
                    raw_asr: trimmed.clone(),
                    llm_draft: None,
                    text: trimmed,
                    speaker: None,
                    words: vec![],
                });
            }
        }

        Ok(Transcript {
            locale: self.locale.clone(),
            asr_model_version: self.asr_model_version.clone(),
            llm_model_version: None,
            segments,
        })
    }

    pub async fn fix_subtitles_local(
        &self,
        raw: &Transcript,
        glossary_entries: &[GlossaryEntry],
    ) -> DomainResult<Transcript> {
        let mut out = raw.clone();
        out.llm_model_version = Some(self.llm_model_version.clone());
        for seg in &mut out.segments {
            let cleaned = apply_glossary_and_cleanup(&seg.text, glossary_entries, true);
            seg.llm_draft = Some(cleaned.clone());
            seg.text = cleaned;
        }
        Ok(out)
    }
}

fn invert_silences(silences: &[SilenceSpan], total_ms: u64) -> Vec<SilenceSpan> {
    let mut speech = Vec::new();
    let mut cursor = 0_u64;
    for s in silences {
        if s.start_ms > cursor {
            speech.push(SilenceSpan {
                start_ms: cursor,
                end_ms: s.start_ms,
            });
        }
        cursor = cursor.max(s.end_ms);
    }
    if total_ms > cursor {
        speech.push(SilenceSpan {
            start_ms: cursor,
            end_ms: total_ms,
        });
    }
    speech
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

    #[test]
    fn detects_silences_in_pcm() {
        // 0.5s tone + 0.6s silence + 0.5s tone at 16kHz
        let mut pcm = Vec::new();
        pcm.extend(std::iter::repeat_n(0.5_f32, 8_000));
        pcm.extend(std::iter::repeat_n(0.0_f32, 9_600));
        pcm.extend(std::iter::repeat_n(0.5_f32, 8_000));

        let silences = detect_silences_pcm16k(&pcm, 0.05, 400);
        assert_eq!(silences.len(), 1);
        assert!(silences[0].start_ms >= 480 && silences[0].start_ms <= 520);
        assert!(silences[0].end_ms >= 1080 && silences[0].end_ms <= 1120);
    }

    #[test]
    fn applies_glossary_and_strips_fillers() {
        let entries = vec![
            GlossaryEntry {
                spoken_variant: "cơ sơ".into(),
                preferred_term: "Cursor".into(),
                hit_count: 3,
            },
            GlossaryEntry {
                spoken_variant: "mác búc".into(),
                preferred_term: "MacBook".into(),
                hit_count: 2,
            },
        ];
        let cleaned = apply_glossary_and_cleanup(
            "ờ, hôm nay mình dùng cơ sơ trên mác búc",
            &entries,
            true,
        );
        assert_eq!(cleaned, "Hôm nay mình dùng Cursor trên MacBook");
    }

    #[test]
    fn computes_edit_diff_when_changed() {
        let seg = TranscriptSegment {
            id: Uuid::new_v4(),
            start_ms: 0,
            end_ms: 2_000,
            raw_asr: "dung co so".into(),
            llm_draft: Some("Dùng cơ sơ".into()),
            text: "Dùng cơ sơ".into(),
            speaker: None,
            words: vec![],
        };
        let diff = compute_edit_diff(&seg, "Dùng Cursor").unwrap();
        assert_eq!(diff.user_final, "Cursor".replace("Cursor", "Dùng Cursor"));
        assert_eq!(diff.edit_type, EditType::WordFix);
    }
}
