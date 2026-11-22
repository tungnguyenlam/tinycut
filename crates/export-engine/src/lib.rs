use tinycut_core_domain::{MediaRef, Transcript};

fn format_srt_timestamp(ms: u64) -> String {
    let hours = ms / 3_600_000;
    let rem = ms % 3_600_000;
    let minutes = rem / 60_000;
    let rem = rem % 60_000;
    let seconds = rem / 1_000;
    let millis = rem % 1_000;
    format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}")
}

fn format_ass_timestamp(ms: u64) -> String {
    let hours = ms / 3_600_000;
    let rem = ms % 3_600_000;
    let minutes = rem / 60_000;
    let rem = rem % 60_000;
    let seconds = rem / 1_000;
    let centis = (rem % 1_000) / 10;
    format!("{hours}:{minutes:02}:{seconds:02}.{centis:02}")
}

fn format_smpte_timecode(ms: u64, fps: f32) -> String {
    let fps_int = fps.round().max(1.0) as u64;
    let total_frames = ((ms as f64 / 1000.0) * fps_int as f64).round() as u64;
    let frames = total_frames % fps_int;
    let total_secs = total_frames / fps_int;
    let secs = total_secs % 60;
    let mins = (total_secs / 60) % 60;
    let hours = total_secs / 3600;
    format!("{hours:02}:{mins:02}:{secs:02}:{frames:02}")
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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

/// Exports a `Transcript` to Advanced SubStation Alpha (`.ass`) format.
pub fn export_ass(transcript: &Transcript) -> String {
    let mut out = String::from(
        "[Script Info]\n\
         Title: TinyCut Subtitles\n\
         ScriptType: v4.00+\n\
         PlayResX: 1920\n\
         PlayResY: 1080\n\n\
         [V4+ Styles]\n\
         Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
         Style: Default,Inter,54,&H00FFFFFF,&H000000FF,&H00000000,&H64000000,0,0,0,0,100,100,0,0,1,2,1,2,40,40,48,1\n\n\
         [Events]\n\
         Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );

    for seg in &transcript.segments {
        let text = seg.text.trim().replace('\n', "\\N");
        let speaker = seg.speaker.as_deref().unwrap_or("");
        out.push_str(&format!(
            "Dialogue: 0,{},{},Default,{},0,0,0,,{}\n",
            format_ass_timestamp(seg.start_ms),
            format_ass_timestamp(seg.end_ms),
            speaker,
            text
        ));
    }
    out
}

/// Exports a `Transcript` to Final Cut Pro XML (`.fcpxml` v1.10) with rational millisecond timebases.
pub fn export_fcpxml(transcript: &Transcript, media: &MediaRef) -> String {
    let width = if media.width > 0 { media.width } else { 1920 };
    let height = if media.height > 0 { media.height } else { 1080 };
    let fps = if media.fps > 0.0 {
        media.fps.round().max(1.0) as u64
    } else {
        30
    };
    let total_ms = media
        .duration_ms
        .max(transcript.segments.last().map(|s| s.end_ms).unwrap_or(0));

    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE fcpxml>\n\
         <fcpxml version=\"1.10\">\n\
           <resources>\n\
             <format id=\"r1\" name=\"FFVideoFormat{}p{}\" frameDuration=\"1/{}s\" width=\"{}\" height=\"{}\"/>\n\
             <effect id=\"r2\" name=\"Basic Title\" uid=\".../Titles.localized/Bumper:Opener.localized/Basic Title.localized/Basic Title.moti\"/>\n\
           </resources>\n\
           <library>\n\
             <event name=\"TinyCut Export\">\n\
               <project name=\"TinyCut Subtitles\">\n\
                 <sequence format=\"r1\" duration=\"{}/1000s\">\n\
                   <spine>\n\
                     <gap name=\"Subtitle Track\" offset=\"0/1000s\" duration=\"{}/1000s\">\n",
        height, fps, fps, width, height, total_ms, total_ms
    );

    for (idx, seg) in transcript.segments.iter().enumerate() {
        let dur_ms = seg.end_ms.saturating_sub(seg.start_ms).max(1);
        let escaped = escape_xml(seg.text.trim());
        out.push_str(&format!(
            "                       <title ref=\"r2\" lane=\"1\" name=\"Sub {}\" offset=\"{}/1000s\" duration=\"{}/1000s\" start=\"{}/1000s\">\n\
                                     <text><text-style ref=\"ts{}\">{}</text-style></text>\n\
                                     <text-style-def id=\"ts{}\"><text-style font=\"Inter\" fontSize=\"48\" alignment=\"center\"/></text-style-def>\n\
                                   </title>\n",
            idx + 1,
            seg.start_ms,
            dur_ms,
            seg.start_ms,
            idx + 1,
            escaped,
            idx + 1
        ));
    }

    out.push_str(
        "                     </gap>\n\
                   </spine>\n\
                 </sequence>\n\
               </project>\n\
             </event>\n\
           </library>\n\
         </fcpxml>\n",
    );
    out
}

/// Exports a `Transcript` to Premiere Pro / DaVinci Resolve XMEML v5 (`.xml`) marker track.
pub fn export_premiere_xml(transcript: &Transcript, media: &MediaRef) -> String {
    let timebase = if media.fps > 0.0 {
        media.fps.round().max(1.0) as u64
    } else {
        30
    };
    let total_ms = media
        .duration_ms
        .max(transcript.segments.last().map(|s| s.end_ms).unwrap_or(0));
    let total_frames = ((total_ms as f64 / 1000.0) * timebase as f64).round() as u64;

    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <xmeml version=\"5\">\n\
           <sequence>\n\
             <name>TinyCut Subtitles</name>\n\
             <duration>{total_frames}</duration>\n\
             <rate><timebase>{timebase}</timebase><ntsc>FALSE</ntsc></rate>\n"
    );

    for (idx, seg) in transcript.segments.iter().enumerate() {
        let in_frame = ((seg.start_ms as f64 / 1000.0) * timebase as f64).round() as u64;
        let out_frame = ((seg.end_ms as f64 / 1000.0) * timebase as f64).round() as u64;
        let escaped = escape_xml(seg.text.trim());
        out.push_str(&format!(
            "             <marker>\n\
                           <name>Sub {}</name>\n\
                           <comment>{}</comment>\n\
                           <in>{}</in>\n\
                           <out>{}</out>\n\
                         </marker>\n",
            idx + 1,
            escaped,
            in_frame,
            out_frame
        ));
    }

    out.push_str("           </sequence>\n</xmeml>\n");
    out
}

/// Exports a `Transcript` to CMX 3600 EDL (`.edl`) for DaVinci Resolve and Premiere Pro.
pub fn export_edl(transcript: &Transcript, fps: f32) -> String {
    let mut out = String::from("TITLE: TinyCut Subtitles\nFCM: NON-DROP FRAME\n\n");
    for (idx, seg) in transcript.segments.iter().enumerate() {
        let start_tc = format_smpte_timecode(seg.start_ms, fps);
        let end_tc = format_smpte_timecode(seg.end_ms, fps);
        out.push_str(&format!(
            "{:03}  AX       V     C        {} {} {} {}\n* FROM CLIP NAME: {}\n\n",
            idx + 1,
            start_tc,
            end_tc,
            start_tc,
            end_tc,
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

    fn sample_transcript() -> Transcript {
        Transcript {
            locale: "vi-VN".into(),
            asr_model_version: "whisper-v3-turbo".into(),
            llm_model_version: Some("qwen3-1.7b".into()),
            segments: vec![TranscriptSegment {
                id: Uuid::new_v4(),
                start_ms: 1_250,
                end_ms: 4_500,
                raw_asr: "hom nay minh tren tay con macbook".into(),
                llm_draft: None,
                text: "Hôm nay mình trên tay con MacBook & Air.".into(),
                speaker: Some("Host".into()),
                words: vec![],
            }],
        }
    }

    fn sample_media() -> MediaRef {
        MediaRef {
            id: Uuid::new_v4(),
            path: "/tmp/demo.mp4".into(),
            duration_ms: 10_000,
            width: 3840,
            height: 2160,
            fps: 60.0,
        }
    }

    #[test]
    fn exports_valid_srt() {
        let srt = export_srt(&sample_transcript());
        assert!(srt.contains(
            "1\n00:00:01,250 --> 00:00:04,500\nHôm nay mình trên tay con MacBook & Air."
        ));
    }

    #[test]
    fn exports_valid_ass() {
        let ass = export_ass(&sample_transcript());
        assert!(ass.contains("[Events]"));
        assert!(ass.contains("Dialogue: 0,0:00:01.25,0:00:04.50,Default,Host,0,0,0,,Hôm nay mình trên tay con MacBook & Air."));
    }

    #[test]
    fn exports_valid_fcpxml_with_escaping() {
        let xml = export_fcpxml(&sample_transcript(), &sample_media());
        assert!(xml.contains("<fcpxml version=\"1.10\">"));
        assert!(xml.contains("offset=\"1250/1000s\" duration=\"3250/1000s\""));
        assert!(xml.contains("MacBook &amp; Air."));
    }

    #[test]
    fn exports_valid_premiere_xml_and_edl() {
        let xml = export_premiere_xml(&sample_transcript(), &sample_media());
        assert!(xml.contains("<xmeml version=\"5\">"));
        assert!(xml.contains("<timebase>60</timebase>"));
        assert!(xml.contains("<in>75</in>"));

        let edl = export_edl(&sample_transcript(), 30.0);
        assert!(edl.contains("TITLE: TinyCut Subtitles"));
        assert!(edl.contains("001  AX       V     C        00:00:01:08 00:00:04:15"));
    }
}
