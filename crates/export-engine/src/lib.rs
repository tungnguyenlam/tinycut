use tinycut_core_domain::Transcript;

fn format_srt_timestamp(ms: u64) -> String {
    let hours = ms / 3_600_000;
    let rem = ms % 3_600_000;
    let minutes = rem / 60_000;
    let rem = rem % 60_000;
    let seconds = rem / 1_000;
    let millis = rem % 1_000;
    format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}")
}

/// Exports a `Transcript` to standard SubRip (`.srt`) format.
pub fn export_srt(transcript: &Transcript) -> String {
    let mut out = String::new();
    for (idx, seg) in transcript.segments.iter().enumerate() {
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            idx + 1,
            format_srt_timestamp(seg.start_ms),
            format_srt_timestamp(seg.end_ms),
            seg.text.trim()
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinycut_core_domain::TranscriptSegment;
    use uuid::Uuid;

    #[test]
    fn exports_valid_srt() {
        let t = Transcript {
            locale: "vi-VN".into(),
            asr_model_version: "whisper-v3-turbo".into(),
            llm_model_version: Some("qwen3-1.7b".into()),
            segments: vec![TranscriptSegment {
                id: Uuid::new_v4(),
                start_ms: 1_250,
                end_ms: 4_500,
                raw_asr: "hom nay minh tren tay con macbook".into(),
                llm_draft: None,
                text: "Hôm nay mình trên tay con MacBook.".into(),
                speaker: None,
                words: vec![],
            }],
        };
        let srt = export_srt(&t);
        assert!(srt.contains("1\n00:00:01,250 --> 00:00:04,500\nHôm nay mình trên tay con MacBook."));
    }
}
