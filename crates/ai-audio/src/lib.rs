use std::path::{Path, PathBuf};
#[cfg(any(feature = "whisper", feature = "vad-onnx", feature = "local-llm"))]
use tinycut_core_domain::DomainError;
use tinycut_core_domain::{
    DomainResult, EditDiff, EditType, GlossaryEntry, MediaRef, ModelType, Transcript,
    TranscriptSegment,
};
use tinycut_media_engine::{FfmpegMediaExtractor, MediaExtractor};
use tinycut_model_hub::ModelManager;
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

/// Detects silence regions in 16kHz mono PCM using RMS energy thresholding (zero-download fallback
/// when Silero-VAD ONNX weights are not installed yet).
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

/// Runs neural Voice Activity Detection using the default lightweight **Silero-VAD v5** ONNX model
/// (`silero_vad.onnx`, 16 kHz mono PCM, 512-sample windows with 64-sample rolling context).
#[cfg(feature = "vad-onnx")]
pub fn detect_silences_silero_vad(
    pcm_16k: &[f32],
    model_path: &Path,
    speech_threshold: f32,
    min_silence_ms: u64,
) -> DomainResult<Vec<SilenceSpan>> {
    use ort::session::Session;
    use ort::value::Tensor;

    const SAMPLE_RATE: i64 = 16_000;
    const WINDOW_SAMPLES: usize = 512;
    const CONTEXT_SAMPLES: usize = 64;
    const WINDOW_MS: u64 = 32; // 512 / 16000 = 32 ms

    let env = ort::init()
        .with_name("tinycut-vad")
        .build()
        .map_err(|e| DomainError::Inference(format!("ort environment init failed: {e}")))?;

    let mut session = Session::builder(&env)
        .map_err(|e| DomainError::Inference(format!("ort session builder failed: {e}")))?
        .with_intra_threads(2)
        .map_err(|e| DomainError::Inference(format!("ort threads config failed: {e}")))?
        .commit_from_file(model_path)
        .map_err(|e| {
            DomainError::Inference(format!(
                "failed to load silero-vad ONNX from {}: {e}",
                model_path.display()
            ))
        })?;

    let mut state = vec![0.0_f32; 2 * 128];
    let mut context = [0.0_f32; CONTEXT_SAMPLES];
    let mut spans = Vec::new();
    let mut silence_start: Option<u64> = None;

    for (idx, chunk) in pcm_16k.chunks(WINDOW_SAMPLES).enumerate() {
        let mut window = [0.0_f32; WINDOW_SAMPLES];
        window[..chunk.len()].copy_from_slice(chunk);

        let mut input_buf = Vec::with_capacity(CONTEXT_SAMPLES + WINDOW_SAMPLES);
        input_buf.extend_from_slice(&context);
        input_buf.extend_from_slice(&window);
        context.copy_from_slice(&window[WINDOW_SAMPLES - CONTEXT_SAMPLES..]);

        let input_tensor = Tensor::from_array(([1_usize, CONTEXT_SAMPLES + WINDOW_SAMPLES], input_buf))
            .map_err(|e| DomainError::Inference(format!("vad input tensor error: {e}")))?;
        let state_tensor = Tensor::from_array(([2_usize, 1, 128], state.clone()))
            .map_err(|e| DomainError::Inference(format!("vad state tensor error: {e}")))?;
        let sr_tensor = Tensor::from_array(([1_usize], vec![SAMPLE_RATE]))
            .map_err(|e| DomainError::Inference(format!("vad sr tensor error: {e}")))?;

        let outputs = session
            .run(ort::inputs![
                "input" => input_tensor,
                "state" => state_tensor,
                "sr" => sr_tensor
            ])
            .map_err(|e| DomainError::Inference(format!("silero-vad inference failed: {e}")))?;

        let (_, prob_slice) = outputs["output"]
            .try_extract_tensor::<f32>()
            .map_err(|e| DomainError::Inference(format!("vad output extract error: {e}")))?;
        let speech_prob = prob_slice.first().copied().unwrap_or(0.0);

        let (_, next_state) = outputs["stateN"]
            .try_extract_tensor::<f32>()
            .map_err(|e| DomainError::Inference(format!("vad stateN extract error: {e}")))?;
        if next_state.len() == state.len() {
            state.copy_from_slice(next_state);
        }

        let ms = (idx as u64) * WINDOW_MS;
        if speech_prob < speech_threshold {
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
        let end_ms = ((pcm_16k.len() as u64) * 1000) / SAMPLE_RATE as u64;
        if end_ms.saturating_sub(start) >= min_silence_ms {
            spans.push(SilenceSpan { start_ms: start, end_ms });
        }
    }
    Ok(spans)
}

/// Applies Personal Glossary substitutions (`spoken_variant -> preferred_term`) and basic
/// Vietnamese/English punctuation & filler cleanup.
pub fn apply_glossary_and_cleanup(
    text: &str,
    entries: &[GlossaryEntry],
    strip_fillers: bool,
) -> String {
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
        haystack.replace(needle, replacement)
    }
}

/// Builds the Qwen2.5 ChatML prompt for bilingual Vietnamese/English subtitle cleanup.
pub fn build_subtitle_fixer_prompt(raw_text: &str, glossary_entries: &[GlossaryEntry]) -> String {
    let glossary_block = if glossary_entries.is_empty() {
        String::new()
    } else {
        let pairs: Vec<String> = glossary_entries
            .iter()
            .take(32)
            .map(|e| format!("\"{}\" -> \"{}\"", e.spoken_variant, e.preferred_term))
            .collect();
        format!("\nPersonal glossary replacements: {}", pairs.join("; "))
    };

    format!(
        "<|im_start|>system\n\
         You are a concise bilingual Vietnamese/English video subtitle editor. \
         Fix punctuation, capitalization, speech fillers (ờ, à, ừm, kiểu như), and English tech/brand loanwords while preserving the exact meaning. \
         Output ONLY the cleaned subtitle line with no commentary or quotes.{glossary_block}<|im_end|>\n\
         <|im_start|>user\n{raw_text}<|im_end|>\n\
         <|im_start|>assistant\n"
    )
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

/// Local on-device audio & text inference engine plugged into TinyCut's default lightweight models:
/// - **ASR:** `Whisper Base (Q5_1)` via `whisper-rs`
/// - **VAD:** `Silero-VAD v5` via `ort`
/// - **Subtitle Fixer LLM:** `Qwen2.5-0.5B-Instruct (Q4_K_M)` via `candle`
#[derive(Debug, Clone)]
pub struct LocalAudioEngine {
    pub asr_model_version: String,
    pub vad_model_version: String,
    pub llm_model_version: String,
    pub whisper_model_path: Option<PathBuf>,
    pub vad_model_path: Option<PathBuf>,
    pub llm_model_path: Option<PathBuf>,
    pub llm_tokenizer_path: Option<PathBuf>,
    pub locale: String,
}

impl Default for LocalAudioEngine {
    fn default() -> Self {
        Self::from_model_manager(&ModelManager::default())
    }
}

impl LocalAudioEngine {
    pub fn new(asr_model_version: impl Into<String>, llm_model_version: impl Into<String>) -> Self {
        Self {
            asr_model_version: asr_model_version.into(),
            vad_model_version: "silero-vad-v5".into(),
            llm_model_version: llm_model_version.into(),
            whisper_model_path: None,
            vad_model_path: None,
            llm_model_path: None,
            llm_tokenizer_path: None,
            locale: "vi-VN".into(),
        }
    }

    /// Creates a `LocalAudioEngine` wired automatically to any installed default lightweight models
    /// managed by `mgr` (`~/.tinycut/models` by default).
    pub fn from_model_manager(mgr: &ModelManager) -> Self {
        Self {
            asr_model_version: "whisper-base-q5_1".into(),
            vad_model_version: "silero-vad-v5".into(),
            llm_model_version: "qwen2.5-0.5b-instruct-q4_k_m".into(),
            whisper_model_path: mgr.resolve_if_installed(ModelType::Asr),
            vad_model_path: mgr.resolve_if_installed(ModelType::Vad),
            llm_model_path: mgr.resolve_if_installed(ModelType::TextFixer),
            llm_tokenizer_path: mgr.resolve_if_installed(ModelType::TextFixerTokenizer),
            locale: "vi-VN".into(),
        }
    }

    pub fn with_whisper_model(mut self, path: impl Into<PathBuf>) -> Self {
        self.whisper_model_path = Some(path.into());
        self
    }

    pub fn with_vad_model(mut self, path: impl Into<PathBuf>) -> Self {
        self.vad_model_path = Some(path.into());
        self
    }

    pub fn with_llm_model(
        mut self,
        gguf_path: impl Into<PathBuf>,
        tokenizer_path: impl Into<PathBuf>,
    ) -> Self {
        self.llm_model_path = Some(gguf_path.into());
        self.llm_tokenizer_path = Some(tokenizer_path.into());
        self
    }

    /// Detects silence regions in 16 kHz mono PCM using **Silero-VAD v5 ONNX** when installed,
    /// falling back gracefully to RMS energy thresholding.
    pub fn detect_silences(&self, pcm_16k: &[f32], min_silence_ms: u64) -> Vec<SilenceSpan> {
        #[cfg(feature = "vad-onnx")]
        if let Some(vad_path) = &self.vad_model_path
            && vad_path.exists()
        {
            match detect_silences_silero_vad(pcm_16k, vad_path, 0.5, min_silence_ms) {
                Ok(spans) => return spans,
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        "Silero-VAD ONNX failed; falling back to RMS energy VAD"
                    );
                }
            }
        }
        detect_silences_pcm16k(pcm_16k, 0.015, min_silence_ms)
    }

    /// Runs local transcription:
    /// 1. Extracts 16 kHz mono PCM via `FfmpegMediaExtractor` when `media.path` exists.
    /// 2. Runs `whisper-rs` (`ggml-base-q5_1.bin`) when installed.
    /// 3. Otherwise segments speech spans via **Silero-VAD v5** (or RMS fallback) so the
    ///    pipeline remains functional before ASR weights are downloaded.
    pub async fn transcribe_local(
        &self,
        media: &MediaRef,
        glossary: &[String],
    ) -> DomainResult<Transcript> {
        let prompt = build_glossary_prompt(glossary);
        let path = PathBuf::from(&media.path);

        #[cfg(feature = "whisper")]
        if path.exists()
            && let Some(model_path) = &self.whisper_model_path
            && model_path.exists()
        {
            let extractor = FfmpegMediaExtractor::default();
            let pcm = extractor.extract_pcm_16k(media)?;
            return self.transcribe_with_whisper_rs(&pcm, model_path, &prompt);
        }

        if path.exists() {
            let extractor = FfmpegMediaExtractor::default();
            let pcm = extractor.extract_pcm_16k(media)?;
            let silences = self.detect_silences(&pcm, 400);
            let duration_ms = ((pcm.len() as u64) * 1000) / 16_000;
            let speech_spans = invert_silences(&silences, duration_ms.max(media.duration_ms));
            let vad_tag = if self.vad_model_path.as_ref().is_some_and(|p| p.exists()) {
                "Silero-VAD v5"
            } else {
                "RMS VAD"
            };
            let segments = if speech_spans.is_empty() {
                vec![TranscriptSegment {
                    id: Uuid::new_v4(),
                    start_ms: 0,
                    end_ms: duration_ms.max(1_000),
                    raw_asr: format!(
                        "[Audio detected via {vad_tag}; download default Whisper Base Q5_1 model for ASR. {prompt}]"
                    )
                    .trim()
                    .into(),
                    llm_draft: None,
                    text: format!(
                        "[Audio detected via {vad_tag}; download default Whisper Base Q5_1 model for ASR. {prompt}]"
                    )
                    .trim()
                    .into(),
                    speaker: None,
                    words: vec![],
                }]
            } else {
                speech_spans
                    .into_iter()
                    .enumerate()
                    .map(|(i, span)| {
                        let label = format!(
                            "Speech segment #{} ({vad_tag}; download Whisper Base Q5_1 to transcribe)",
                            i + 1
                        );
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
        model_path: &Path,
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

        let num_segments = state.full_n_segments();
        let mut segments = Vec::with_capacity(num_segments.max(0) as usize);
        for i in 0..num_segments {
            let Some(seg) = state.get_segment(i) else {
                continue;
            };
            let text = seg
                .to_str_lossy()
                .map_err(|e| DomainError::Inference(format!("segment text error: {e}")))?;
            let start_ms = (seg.start_timestamp().max(0) as u64) * 10;
            let end_ms = (seg.end_timestamp().max(0) as u64) * 10;
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

    /// Runs subtitle cleanup using **Qwen2.5-0.5B-Instruct (Q4_K_M GGUF)** via `candle` when both
    /// GGUF and tokenizer files are installed, paired with deterministic Personal Glossary guardrails.
    pub async fn fix_subtitles_local(
        &self,
        raw: &Transcript,
        glossary_entries: &[GlossaryEntry],
    ) -> DomainResult<Transcript> {
        let mut out = raw.clone();

        #[cfg(feature = "local-llm")]
        if let (Some(gguf_path), Some(tok_path)) = (&self.llm_model_path, &self.llm_tokenizer_path)
            && gguf_path.exists()
            && tok_path.exists()
        {
            match self.fix_transcript_with_qwen(&mut out, glossary_entries, gguf_path, tok_path) {
                Ok(()) => {
                    out.llm_model_version = Some(self.llm_model_version.clone());
                    return Ok(out);
                }
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        "Qwen2.5 GGUF inference failed; falling back to deterministic glossary fixer"
                    );
                }
            }
        }

        out.llm_model_version = Some(format!("{}:glossary-fallback", self.llm_model_version));
        for seg in &mut out.segments {
            let cleaned = apply_glossary_and_cleanup(&seg.text, glossary_entries, true);
            seg.llm_draft = Some(cleaned.clone());
            seg.text = cleaned;
        }
        Ok(out)
    }

    #[cfg(feature = "local-llm")]
    fn fix_transcript_with_qwen(
        &self,
        transcript: &mut Transcript,
        glossary_entries: &[GlossaryEntry],
        gguf_path: &Path,
        tokenizer_path: &Path,
    ) -> DomainResult<()> {
        use candle_core::quantized::gguf_file;
        use candle_core::{Device, Tensor};
        use candle_transformers::models::quantized_qwen2::ModelWeights;
        use tokenizers::Tokenizer;

        let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(|e| {
            DomainError::Inference(format!(
                "failed to load tokenizer {}: {e}",
                tokenizer_path.display()
            ))
        })?;
        let eos_im_end = tokenizer.token_to_id("<|im_end|>").unwrap_or(151_645);
        let eos_eot = tokenizer.token_to_id("<|endoftext|>").unwrap_or(151_643);

        let device = Device::Cpu;
        let mut file = std::fs::File::open(gguf_path).map_err(|e| {
            DomainError::Inference(format!(
                "failed to open GGUF {}: {e}",
                gguf_path.display()
            ))
        })?;
        let content = gguf_file::Content::read(&mut file)
            .map_err(|e| DomainError::Inference(format!("failed to parse GGUF header: {e}")))?;

        let mut model = ModelWeights::from_gguf(content, &mut file, &device)
            .map_err(|e| DomainError::Inference(format!("failed to load Qwen2.5 GGUF: {e}")))?;

        for seg in &mut transcript.segments {
            if seg.text.trim().is_empty() {
                continue;
            }
            let prompt = build_subtitle_fixer_prompt(&seg.text, glossary_entries);
            let encoding = tokenizer
                .encode(prompt, true)
                .map_err(|e| DomainError::Inference(format!("tokenizer encode failed: {e}")))?;
            let prompt_tokens = encoding.get_ids();
            if prompt_tokens.is_empty() {
                continue;
            }

            let input = Tensor::new(prompt_tokens, &device)
                .and_then(|t| t.unsqueeze(0))
                .map_err(|e| DomainError::Inference(format!("prompt tensor error: {e}")))?;

            let mut logits = model
                .forward(&input, 0)
                .map_err(|e| DomainError::Inference(format!("qwen prefill error: {e}")))?;
            let max_new_tokens = (seg.text.split_whitespace().count() * 3 + 24).clamp(24, 128);
            let mut generated: Vec<u32> = Vec::with_capacity(max_new_tokens);

            for pos in (prompt_tokens.len()..).take(max_new_tokens) {
                let next_token = argmax_last_logit(&logits)?;
                if next_token == eos_im_end || next_token == eos_eot {
                    break;
                }
                generated.push(next_token);

                if let Ok(piece) = tokenizer.decode(&[next_token], true)
                    && piece.contains('\n')
                    && generated.len() > 2
                {
                    break;
                }

                let next_input = Tensor::new(&[next_token], &device)
                    .and_then(|t| t.unsqueeze(0))
                    .map_err(|e| DomainError::Inference(format!("step tensor error: {e}")))?;
                logits = model
                    .forward(&next_input, pos)
                    .map_err(|e| DomainError::Inference(format!("qwen decode error: {e}")))?;
            }

            let decoded = tokenizer
                .decode(&generated, true)
                .map_err(|e| DomainError::Inference(format!("tokenizer decode failed: {e}")))?;
            let first_line = decoded
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or("")
                .trim_matches('"')
                .trim();

            let candidate = if first_line.is_empty() || first_line.len() > seg.text.len() * 3 + 32 {
                apply_glossary_and_cleanup(&seg.text, glossary_entries, true)
            } else {
                apply_glossary_and_cleanup(first_line, glossary_entries, true)
            };

            seg.llm_draft = Some(candidate.clone());
            seg.text = candidate;
        }

        Ok(())
    }
}

#[cfg(feature = "local-llm")]
fn argmax_last_logit(logits: &candle_core::Tensor) -> DomainResult<u32> {
    let flat = match logits.dims() {
        [_b, _seq, _vocab] => {
            let seq_len = logits.dim(1).map_err(|e| DomainError::Inference(e.to_string()))?;
            logits
                .narrow(1, seq_len.saturating_sub(1), 1)
                .and_then(|t| t.squeeze(0))
                .and_then(|t| t.squeeze(0))
        }
        [_b, _vocab] => logits.squeeze(0),
        [_vocab] => Ok(logits.clone()),
        other => {
            return Err(DomainError::Inference(format!(
                "unexpected Qwen logits rank: {other:?}"
            )));
        }
    }
    .map_err(|e| DomainError::Inference(format!("failed to slice logits: {e}")))?;

    let scores: Vec<f32> = flat
        .to_vec1()
        .map_err(|e| DomainError::Inference(format!("failed to read logits vector: {e}")))?;
    let (best_idx, _) = scores
        .iter()
        .enumerate()
        .fold((0usize, f32::NEG_INFINITY), |(bi, bv), (i, &v)| {
            if v > bv { (i, v) } else { (bi, bv) }
        });
    Ok(best_idx as u32)
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
    fn subtitle_fixer_prompt_includes_glossary() {
        let entries = vec![GlossaryEntry {
            spoken_variant: "cơ sơ".into(),
            preferred_term: "Cursor".into(),
            hit_count: 2,
        }];
        let prompt = build_subtitle_fixer_prompt("hôm nay mình dùng cơ sơ", &entries);
        assert!(prompt.contains("\"cơ sơ\" -> \"Cursor\""));
        assert!(prompt.contains("<|im_start|>assistant"));
    }

    #[test]
    fn detects_silences_in_pcm() {
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
        assert_eq!(diff.user_final, "Dùng Cursor");
        assert_eq!(diff.edit_type, EditType::WordFix);
    }

    #[test]
    #[ignore = "downloads Silero-VAD v5 + Whisper Base Q5_1 into default cache and runs live inference"]
    fn live_silero_vad_and_whisper_smoke_test() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mgr = ModelManager::default();
        let vad_path = rt.block_on(mgr.ensure_model(ModelType::Vad)).unwrap();
        let whisper_path = rt.block_on(mgr.ensure_model(ModelType::Asr)).unwrap();

        let pcm = vec![0.0_f32; 16_000]; // 1s silence at 16 kHz
        #[cfg(feature = "vad-onnx")]
        {
            let silences = detect_silences_silero_vad(&pcm, &vad_path, 0.5, 400).unwrap();
            assert!(!silences.is_empty(), "1s silence should be detected by Silero-VAD");
        }

        let engine = LocalAudioEngine::from_model_manager(&mgr);
        assert_eq!(engine.whisper_model_path.as_deref(), Some(whisper_path.as_path()));
        #[cfg(feature = "whisper")]
        {
            let tr = engine
                .transcribe_with_whisper_rs(&pcm, &whisper_path, "Glossary terms: TinyCut.")
                .unwrap();
            assert_eq!(tr.asr_model_version, "whisper-base-q5_1");
        }
    }

    #[test]
    #[ignore = "downloads Qwen2.5-0.5B-Instruct Q4_K_M GGUF + tokenizer (~405MB) and runs live inference"]
    fn live_qwen_subtitle_fixer_smoke_test() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mgr = ModelManager::default();
        rt.block_on(mgr.ensure_task_bundle(ModelType::TextFixer))
            .unwrap();

        let engine = LocalAudioEngine::from_model_manager(&mgr);
        let raw = Transcript {
            locale: "vi-VN".into(),
            asr_model_version: "whisper-base-q5_1".into(),
            llm_model_version: None,
            segments: vec![TranscriptSegment {
                id: Uuid::new_v4(),
                start_ms: 0,
                end_ms: 2_500,
                raw_asr: "ờ, hôm nay mình review cơ sơ trên mác búc".into(),
                llm_draft: None,
                text: "ờ, hôm nay mình review cơ sơ trên mác búc".into(),
                speaker: None,
                words: vec![],
            }],
        };
        let glossary = vec![
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

        let fixed = rt.block_on(engine.fix_subtitles_local(&raw, &glossary)).unwrap();
        assert_eq!(
            fixed.llm_model_version.as_deref(),
            Some("qwen2.5-0.5b-instruct-q4_k_m")
        );
        let text = &fixed.segments[0].text;
        println!("Qwen2.5 fixed subtitle: {text}");
        assert!(!text.is_empty());
    }
}
