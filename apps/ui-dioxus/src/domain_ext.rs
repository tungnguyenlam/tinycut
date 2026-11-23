use tinycut_core_domain::{
    MediaRef, NormalizedBox, RedactionTrack, ShieldConfig, Transcript,
};

/// Formats milliseconds as SMPTE timecode `HH:MM:SS:FF` for the NLE transport bar.
pub fn format_smpte(ms: u64, fps: f32) -> String {
    let fps_int = fps.round().max(1.0) as u64;
    let total_frames = ((ms as f64 / 1000.0) * fps_int as f64).floor() as u64;
    let frames = total_frames % fps_int;
    let total_secs = total_frames / fps_int;
    let secs = total_secs % 60;
    let mins = (total_secs / 60) % 60;
    let hours = total_secs / 3600;
    format!("{hours:02}:{mins:02}:{secs:02}:{frames:02}")
}

/// Formats milliseconds for subtitle rows (`MM:SS.mmm`, or `HH:MM:SS.mmm` if >= 1 hour).
pub fn format_seg_time(ms: u64) -> String {
    let hours = ms / 3_600_000;
    let rem = ms % 3_600_000;
    let mins = rem / 60_000;
    let rem = rem % 60_000;
    let secs = rem / 1_000;
    let millis = rem % 1_000;
    if hours > 0 {
        format!("{hours:02}:{mins:02}:{secs:02}.{millis:03}")
    } else {
        format!("{mins:02}:{secs:02}.{millis:03}")
    }
}

pub fn ms_to_frame(ms: u64, fps: f32) -> u64 {
    ((ms as f64 / 1000.0) * fps.max(1.0) as f64).floor() as u64
}

pub fn frame_to_ms(frame: u64, fps: f32) -> u64 {
    ((frame as f64 / fps.max(1.0) as f64) * 1000.0).round() as u64
}

// ============================================================================
// 3-Way Word Diff (Raw ASR -> LLM Draft -> User Final) + Glossary Candidates
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffOrigin {
    Unchanged,
    DeletedAsr,
    InsertedLlm,
    InsertedUser,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffToken {
    pub text: String,
    pub origin: DiffOrigin,
    pub is_filler: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlossaryCandidate {
    pub spoken_variant: String,
    pub preferred_term: String,
}

const FILLER_PHRASES: &[&str] = &[
    "kiểu như ",
    "thì là ",
    "ờ, ",
    "à, ",
    "ừm, ",
    "ừ, ",
    "uh, ",
    "um, ",
];

const FILLER_SINGLE: &[&str] = &["ờ", "ờ,", "à", "à,", "ừm", "ừm,", "ừ", "uh", "um"];

pub fn is_filler_token(tok: &str) -> bool {
    let clean = tok.trim().to_lowercase();
    FILLER_SINGLE.contains(&clean.as_str())
}

pub fn count_fillers_in_text(text: &str) -> usize {
    let lower = text.to_lowercase();
    let mut count = 0;
    for phrase in ["kiểu như", "thì là"] {
        count += lower.matches(phrase).count();
    }
    for word in lower.split_whitespace() {
        if FILLER_SINGLE.contains(&word) {
            count += 1;
        }
    }
    count
}

pub fn strip_fillers_text(text: &str) -> String {
    let mut out = text.to_string();
    for phrase in FILLER_PHRASES {
        out = replace_ci(&out, phrase, "");
    }
    let tokens: Vec<&str> = out
        .split_whitespace()
        .filter(|w| !is_filler_token(w))
        .collect();
    let joined = tokens.join(" ");
    let mut chars = joined.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

fn replace_ci(haystack: &str, needle: &str, replacement: &str) -> String {
    let lh = haystack.to_lowercase();
    let ln = needle.to_lowercase();
    if ln.is_empty() || !lh.contains(&ln) {
        return haystack.to_string();
    }
    if lh.len() == haystack.len() {
        let mut out = String::with_capacity(haystack.len());
        let mut cur = 0;
        while let Some(pos) = lh[cur..].find(&ln) {
            let abs = cur + pos;
            out.push_str(&haystack[cur..abs]);
            out.push_str(replacement);
            cur = abs + ln.len();
        }
        out.push_str(&haystack[cur..]);
        out
    } else {
        haystack.replace(needle, replacement)
    }
}

fn normalize_word(w: &str) -> String {
    w.trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
}

/// Computes a Unicode-aware LCS word diff between `raw_asr`, optional `llm_draft`, and `user_final`.
pub fn compute_three_way_diff(
    raw_asr: &str,
    llm_draft: Option<&str>,
    user_final: &str,
) -> (Vec<DiffToken>, Vec<GlossaryCandidate>) {
    let raw_words: Vec<&str> = raw_asr.split_whitespace().collect();
    let final_words: Vec<&str> = user_final.split_whitespace().collect();
    let llm_set: Vec<String> = llm_draft
        .unwrap_or("")
        .split_whitespace()
        .map(normalize_word)
        .collect();

    let n = raw_words.len();
    let m = final_words.len();
    let mut dp = vec![vec![0usize; m + 1]; n + 1];

    for i in (0..n).rev() {
        let rw = normalize_word(raw_words[i]);
        for j in (0..m).rev() {
            if !rw.is_empty() && rw == normalize_word(final_words[j]) {
                dp[i][j] = dp[i + 1][j + 1] + 1;
            } else {
                dp[i][j] = dp[i + 1][j].max(dp[i][j + 1]);
            }
        }
    }

    let mut tokens = Vec::new();
    let mut candidates = Vec::new();
    let mut i = 0;
    let mut j = 0;

    while i < n || j < m {
        if i < n
            && j < m
            && !normalize_word(raw_words[i]).is_empty()
            && normalize_word(raw_words[i]) == normalize_word(final_words[j])
        {
            let text = final_words[j].to_string();
            let filler = is_filler_token(&text);
            tokens.push(DiffToken {
                text,
                origin: DiffOrigin::Unchanged,
                is_filler: filler,
            });
            i += 1;
            j += 1;
        } else {
            // Collect a contiguous replacement span (deleted raw tokens + inserted final tokens)
            let mut del_span = Vec::new();
            let mut ins_span = Vec::new();

            while i < n || j < m {
                if i < n
                    && j < m
                    && !normalize_word(raw_words[i]).is_empty()
                    && normalize_word(raw_words[i]) == normalize_word(final_words[j])
                {
                    break;
                }
                if i < n && (j == m || dp[i + 1][j] >= dp[i][j + 1]) {
                    let w = raw_words[i].to_string();
                    let filler = is_filler_token(&w);
                    if !filler {
                        del_span.push(
                            w.trim_matches(|c: char| matches!(c, ',' | '.' | '!' | '?'))
                                .to_string(),
                        );
                    }
                    tokens.push(DiffToken {
                        text: w,
                        origin: DiffOrigin::DeletedAsr,
                        is_filler: filler,
                    });
                    i += 1;
                } else if j < m {
                    let w = final_words[j].to_string();
                    let norm = normalize_word(&w);
                    let in_llm = llm_draft.is_some() && llm_set.contains(&norm);
                    let filler = is_filler_token(&w);
                    ins_span.push(
                        w.trim_matches(|c: char| matches!(c, ',' | '.' | '!' | '?'))
                            .to_string(),
                    );
                    tokens.push(DiffToken {
                        text: w,
                        origin: if in_llm {
                            DiffOrigin::InsertedLlm
                        } else {
                            DiffOrigin::InsertedUser
                        },
                        is_filler: filler,
                    });
                    j += 1;
                }
            }

            if !del_span.is_empty()
                && !ins_span.is_empty()
                && del_span.len() <= 3
                && ins_span.len() <= 3
            {
                let spoken = del_span.join(" ");
                let preferred = ins_span.join(" ");
                if !spoken.eq_ignore_ascii_case(&preferred)
                    && !spoken.is_empty()
                    && !preferred.is_empty()
                {
                    candidates.push(GlossaryCandidate {
                        spoken_variant: spoken,
                        preferred_term: preferred,
                    });
                }
            }
        }
    }

    (tokens, candidates)
}

// ============================================================================
// Spoken PII Detection (VN phone numbers, emails, API keys, CCCD)
// ============================================================================

pub fn detect_spoken_pii(text: &str) -> Option<&'static str> {
    if text.contains('@') && text.contains('.') {
        return Some("Email");
    }
    if text.contains("sk-") || text.contains("AIza") {
        return Some("API Key");
    }
    let digits_only: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits_only.len() >= 9 {
        if digits_only.starts_with("09")
            || digits_only.starts_with("08")
            || digits_only.starts_with("03")
            || digits_only.starts_with("07")
            || digits_only.starts_with("84")
        {
            return Some("Phone #");
        }
        return Some("ID / Number");
    }
    None
}

// ============================================================================
// Redaction Track Interpolation + Zero-Leak Padding
// ============================================================================

#[derive(Debug, Clone, Copy)]
pub struct InterpolatedBox {
    pub raw: NormalizedBox,
    pub padded: NormalizedBox,
    pub confidence: f32,
}

/// Returns `(first_frame, last_frame)` of a track's keyframes, plus temporally padded bounds.
pub fn track_frame_bounds(track: &RedactionTrack, cfg: &ShieldConfig, total_frames: u64) -> Option<(u64, u64, u64, u64)> {
    let first = track.keyframes.first()?.frame_index;
    let last = track.keyframes.last()?.frame_index;
    let pad = cfg.temporal_padding_frames as u64;
    let pad_start = first.saturating_sub(pad);
    let pad_end = (last + pad).min(total_frames.max(last));
    Some((first, last, pad_start, pad_end))
}

/// Linearly interpolates a single track at `frame_index` honoring both temporal padding (`±3f`)
/// and spatial Zero-Leak padding (`+10%`), returning both raw & padded boxes for live margin preview.
pub fn interpolate_track_at(
    track: &RedactionTrack,
    frame_index: u64,
    cfg: &ShieldConfig,
) -> Option<InterpolatedBox> {
    let kfs = &track.keyframes;
    if kfs.is_empty() {
        return None;
    }
    let pad = cfg.temporal_padding_frames as u64;
    let first = kfs[0];
    let last = kfs[kfs.len() - 1];

    if frame_index + pad < first.frame_index || frame_index > last.frame_index + pad {
        return None;
    }

    let (raw, confidence) = if frame_index <= first.frame_index {
        (first.bbox, first.confidence)
    } else if frame_index >= last.frame_index {
        (last.bbox, last.confidence)
    } else {
        match kfs.binary_search_by_key(&frame_index, |k| k.frame_index) {
            Ok(idx) => (kfs[idx].bbox, kfs[idx].confidence),
            Err(next_idx) => {
                let prev = &kfs[next_idx - 1];
                let next = &kfs[next_idx];
                let span = (next.frame_index - prev.frame_index).max(1) as f32;
                let t = (frame_index - prev.frame_index) as f32 / span;
                (
                    NormalizedBox {
                        x: prev.bbox.x + (next.bbox.x - prev.bbox.x) * t,
                        y: prev.bbox.y + (next.bbox.y - prev.bbox.y) * t,
                        w: prev.bbox.w + (next.bbox.w - prev.bbox.w) * t,
                        h: prev.bbox.h + (next.bbox.h - prev.bbox.h) * t,
                    },
                    prev.confidence + (next.confidence - prev.confidence) * t,
                )
            }
        }
    };

    let padded = raw.expand(cfg.spatial_padding_ratio);
    Some(InterpolatedBox {
        raw,
        padded,
        confidence,
    })
}

// ============================================================================
// Client-side Multi-Format Export Previewer (mirrors export-engine for web/preview)
// ============================================================================

fn format_srt_ts(ms: u64) -> String {
    let h = ms / 3_600_000;
    let rem = ms % 3_600_000;
    let m = rem / 60_000;
    let rem = rem % 60_000;
    let s = rem / 1_000;
    let mi = rem % 1_000;
    format!("{h:02}:{m:02}:{s:02},{mi:03}")
}

fn format_ass_ts(ms: u64) -> String {
    let h = ms / 3_600_000;
    let rem = ms % 3_600_000;
    let m = rem / 60_000;
    let rem = rem % 60_000;
    let s = rem / 1_000;
    let cs = (rem % 1_000) / 10;
    format!("{h}:{m:02}:{s:02}.{cs:02}")
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn wrap_chars(text: &str, max_chars: usize) -> String {
    if max_chars == 0 || text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let next_len = if current.is_empty() {
            word.chars().count()
        } else {
            current.chars().count() + 1 + word.chars().count()
        };
        if next_len > max_chars && !current.is_empty() {
            lines.push(current);
            current = word.to_string();
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines.join("\n")
}

pub fn prepare_export_transcript(
    transcript: &Transcript,
    strip_fillers: bool,
    include_speaker: bool,
    max_chars_per_line: usize,
) -> Transcript {
    let mut out = transcript.clone();
    for seg in &mut out.segments {
        let mut txt = if strip_fillers {
            strip_fillers_text(&seg.text)
        } else {
            seg.text.trim().to_string()
        };
        txt = wrap_chars(&txt, max_chars_per_line);
        if include_speaker
            && let Some(sp) = seg.speaker.as_deref().filter(|s| !s.is_empty())
        {
            txt = format!("[{sp}] {txt}");
        }
        seg.text = txt;
    }
    out
}

pub fn render_export_local(transcript: &Transcript, media: &MediaRef, format: &str) -> String {
    match format {
        "ass" => {
            let mut out = String::from(
                "[Script Info]\nTitle: TinyCut Subtitles\nScriptType: v4.00+\nPlayResX: 1920\nPlayResY: 1080\n\n\
                 [V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, OutlineColour, BackColour, Bold, Alignment, MarginV\n\
                 Style: Default,Be Vietnam Pro,54,&H00FFFFFF,&H00000000,&H64000000,0,2,48\n\n\
                 [Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
            );
            for seg in &transcript.segments {
                let text = seg.text.trim().replace('\n', "\\N");
                let sp = seg.speaker.as_deref().unwrap_or("");
                out.push_str(&format!(
                    "Dialogue: 0,{},{},Default,{},0,0,0,,{}\n",
                    format_ass_ts(seg.start_ms),
                    format_ass_ts(seg.end_ms),
                    sp,
                    text
                ));
            }
            out
        }
        "fcpxml" => {
            let w = media.width.max(1920);
            let h = media.height.max(1080);
            let fps = media.fps.round().max(24.0) as u64;
            let dur = media.duration_ms.max(1000);
            let mut out = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE fcpxml>\n<fcpxml version=\"1.10\">\n  <resources>\n    <format id=\"r1\" name=\"FFVideoFormat{h}p{fps}\" frameDuration=\"1/{fps}s\" width=\"{w}\" height=\"{h}\"/>\n  </resources>\n  <library>\n    <event name=\"TinyCut Export\">\n      <project name=\"TinyCut Subtitles\">\n        <sequence format=\"r1\" duration=\"{dur}/1000s\">\n          <spine>\n            <gap name=\"Subtitle Track\" offset=\"0/1000s\" duration=\"{dur}/1000s\">\n"
            );
            for (i, seg) in transcript.segments.iter().enumerate() {
                let d = seg.end_ms.saturating_sub(seg.start_ms).max(1);
                out.push_str(&format!(
                    "              <title lane=\"1\" name=\"Sub {}\" offset=\"{}/1000s\" duration=\"{}/1000s\">\n                <text>{}</text>\n              </title>\n",
                    i + 1,
                    seg.start_ms,
                    d,
                    escape_xml(&seg.text)
                ));
            }
            out.push_str("            </gap>\n          </spine>\n        </sequence>\n      </project>\n    </event>\n  </library>\n</fcpxml>\n");
            out
        }
        "premiere_xml" => {
            let tb = media.fps.round().max(24.0) as u64;
            let total_f = ((media.duration_ms as f64 / 1000.0) * tb as f64).round() as u64;
            let mut out = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<xmeml version=\"5\">\n  <sequence>\n    <name>TinyCut Subtitles</name>\n    <duration>{total_f}</duration>\n    <rate><timebase>{tb}</timebase><ntsc>FALSE</ntsc></rate>\n"
            );
            for (i, seg) in transcript.segments.iter().enumerate() {
                let inf = ((seg.start_ms as f64 / 1000.0) * tb as f64).round() as u64;
                let outf = ((seg.end_ms as f64 / 1000.0) * tb as f64).round() as u64;
                out.push_str(&format!(
                    "    <marker>\n      <name>Sub {}</name>\n      <comment>{}</comment>\n      <in>{inf}</in>\n      <out>{outf}</out>\n    </marker>\n",
                    i + 1,
                    escape_xml(&seg.text)
                ));
            }
            out.push_str("  </sequence>\n</xmeml>\n");
            out
        }
        "edl" => {
            let mut out = String::from("TITLE: TinyCut Subtitles\nFCM: NON-DROP FRAME\n\n");
            let fps = media.fps.max(24.0);
            for (i, seg) in transcript.segments.iter().enumerate() {
                let s = format_smpte(seg.start_ms, fps);
                let e = format_smpte(seg.end_ms, fps);
                out.push_str(&format!(
                    "{:03}  AX       V     C        {s} {e} {s} {e}\n* FROM CLIP NAME: {}\n\n",
                    i + 1,
                    seg.text.replace('\n', " ")
                ));
            }
            out
        }
        _ => {
            let mut out = String::new();
            for (i, seg) in transcript.segments.iter().enumerate() {
                out.push_str(&format!(
                    "{}\n{} --> {}\n{}\n\n",
                    i + 1,
                    format_srt_ts(seg.start_ms),
                    format_srt_ts(seg.end_ms),
                    seg.text.trim()
                ));
            }
            out
        }
    }
}
