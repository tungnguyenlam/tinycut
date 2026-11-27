import type {
  DefaultModelSpec,
  MediaRef,
  NormalizedBox,
  RedactionTrack,
  ShieldConfig,
  Transcript,
} from "./bindings";

function pad2(n: number): string {
  return String(Math.floor(n)).padStart(2, "0");
}

function pad3(n: number): string {
  return String(Math.floor(n)).padStart(3, "0");
}

/** Formats milliseconds as SMPTE timecode `HH:MM:SS:FF` for the NLE transport bar. */
export function formatSmpte(ms: number, fps: number): string {
  const fpsInt = Math.max(1, Math.round(fps));
  const totalFrames = Math.floor((Math.max(0, ms) / 1000) * fpsInt);
  const frames = totalFrames % fpsInt;
  const totalSecs = Math.floor(totalFrames / fpsInt);
  const secs = totalSecs % 60;
  const mins = Math.floor(totalSecs / 60) % 60;
  const hours = Math.floor(totalSecs / 3600);
  return `${pad2(hours)}:${pad2(mins)}:${pad2(secs)}:${pad2(frames)}`;
}

/** Formats milliseconds for subtitle rows (`MM:SS.mmm`, or `HH:MM:SS.mmm` if >= 1 hour). */
export function formatSegTime(ms: number): string {
  const safe = Math.max(0, Math.floor(ms));
  const hours = Math.floor(safe / 3_600_000);
  let rem = safe % 3_600_000;
  const mins = Math.floor(rem / 60_000);
  rem %= 60_000;
  const secs = Math.floor(rem / 1_000);
  const millis = rem % 1_000;
  if (hours > 0) {
    return `${pad2(hours)}:${pad2(mins)}:${pad2(secs)}.${pad3(millis)}`;
  }
  return `${pad2(mins)}:${pad2(secs)}.${pad3(millis)}`;
}

export function msToFrame(ms: number, fps: number): number {
  return Math.floor((Math.max(0, ms) / 1000) * Math.max(1, fps));
}

export function frameToMs(frame: number, fps: number): number {
  return Math.round((Math.max(0, frame) / Math.max(1, fps)) * 1000);
}

// ============================================================================
// 3-Way Word Diff (Raw ASR -> LLM Draft -> User Final) + Glossary Candidates
// ============================================================================

export type DiffOrigin =
  | "unchanged"
  | "deleted_asr"
  | "inserted_llm"
  | "inserted_user";

export interface DiffToken {
  text: string;
  origin: DiffOrigin;
  is_filler: boolean;
}

export interface GlossaryCandidate {
  spoken_variant: string;
  preferred_term: string;
}

const FILLER_PHRASES = [
  "kiểu như ",
  "thì là ",
  "ờ, ",
  "à, ",
  "ừm, ",
  "ừ, ",
  "uh, ",
  "um, ",
];

const FILLER_SINGLE = new Set([
  "ờ",
  "ờ,",
  "à",
  "à,",
  "ừm",
  "ừm,",
  "ừ",
  "uh",
  "um",
]);

export function isFillerToken(tok: string): boolean {
  return FILLER_SINGLE.has(tok.trim().toLowerCase());
}

export function countFillersInText(text: string): number {
  const lower = text.toLowerCase();
  let count = 0;
  for (const phrase of ["kiểu như", "thì là"]) {
    let idx = 0;
    while ((idx = lower.indexOf(phrase, idx)) !== -1) {
      count += 1;
      idx += phrase.length;
    }
  }
  for (const word of lower.split(/\s+/)) {
    if (FILLER_SINGLE.has(word)) {
      count += 1;
    }
  }
  return count;
}

function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

export function stripFillersText(text: string): string {
  let out = text;
  for (const phrase of FILLER_PHRASES) {
    out = out.replace(new RegExp(escapeRegExp(phrase), "gi"), "");
  }
  const tokens = out
    .split(/\s+/)
    .filter((w) => w.length > 0 && !isFillerToken(w));
  const joined = tokens.join(" ");
  if (!joined) return "";
  const [first, ...rest] = Array.from(joined);
  return first.toUpperCase() + rest.join("");
}

function normalizeWord(w: string): string {
  return w.replace(/^[^\p{L}\p{N}]+|[^\p{L}\p{N}]+$/gu, "").toLowerCase();
}

function trimEdgePunct(w: string): string {
  return w.replace(/^[,.\-!?]+|[,.\-!?]+$/g, "");
}

/** Computes a Unicode-aware LCS word diff between `rawAsr`, optional `llmDraft`, and `userFinal`. */
export function computeThreeWayDiff(
  rawAsr: string,
  llmDraft: string | null,
  userFinal: string
): { tokens: DiffToken[]; candidates: GlossaryCandidate[] } {
  const rawWords = rawAsr.trim() ? rawAsr.trim().split(/\s+/) : [];
  const finalWords = userFinal.trim() ? userFinal.trim().split(/\s+/) : [];
  const llmSet = new Set(
    (llmDraft ?? "")
      .trim()
      .split(/\s+/)
      .filter(Boolean)
      .map(normalizeWord)
  );

  const n = rawWords.length;
  const m = finalWords.length;
  const dp: number[][] = Array.from({ length: n + 1 }, () =>
    new Array<number>(m + 1).fill(0)
  );

  for (let i = n - 1; i >= 0; i--) {
    const rw = normalizeWord(rawWords[i]);
    for (let j = m - 1; j >= 0; j--) {
      if (rw && rw === normalizeWord(finalWords[j])) {
        dp[i][j] = dp[i + 1][j + 1] + 1;
      } else {
        dp[i][j] = Math.max(dp[i + 1][j], dp[i][j + 1]);
      }
    }
  }

  const tokens: DiffToken[] = [];
  const candidates: GlossaryCandidate[] = [];
  let i = 0;
  let j = 0;

  while (i < n || j < m) {
    if (
      i < n &&
      j < m &&
      normalizeWord(rawWords[i]) !== "" &&
      normalizeWord(rawWords[i]) === normalizeWord(finalWords[j])
    ) {
      const text = finalWords[j];
      tokens.push({
        text,
        origin: "unchanged",
        is_filler: isFillerToken(text),
      });
      i++;
      j++;
    } else {
      const delSpan: string[] = [];
      const insSpan: string[] = [];

      while (i < n || j < m) {
        if (
          i < n &&
          j < m &&
          normalizeWord(rawWords[i]) !== "" &&
          normalizeWord(rawWords[i]) === normalizeWord(finalWords[j])
        ) {
          break;
        }
        if (i < n && (j === m || dp[i + 1][j] >= dp[i][j + 1])) {
          const w = rawWords[i];
          const filler = isFillerToken(w);
          if (!filler) {
            delSpan.push(trimEdgePunct(w));
          }
          tokens.push({
            text: w,
            origin: "deleted_asr",
            is_filler: filler,
          });
          i++;
        } else if (j < m) {
          const w = finalWords[j];
          const norm = normalizeWord(w);
          const inLlm = llmDraft !== null && llmSet.has(norm);
          const filler = isFillerToken(w);
          insSpan.push(trimEdgePunct(w));
          tokens.push({
            text: w,
            origin: inLlm ? "inserted_llm" : "inserted_user",
            is_filler: filler,
          });
          j++;
        }
      }

      if (
        delSpan.length > 0 &&
        insSpan.length > 0 &&
        delSpan.length <= 3 &&
        insSpan.length <= 3
      ) {
        const spoken = delSpan.join(" ");
        const preferred = insSpan.join(" ");
        if (
          spoken &&
          preferred &&
          spoken.toLowerCase() !== preferred.toLowerCase()
        ) {
          candidates.push({
            spoken_variant: spoken,
            preferred_term: preferred,
          });
        }
      }
    }
  }

  return { tokens, candidates };
}

// ============================================================================
// Spoken PII Detection (VN phone numbers, emails, API keys, CCCD)
// ============================================================================

export function detectSpokenPii(text: string): string | null {
  if (text.includes("@") && text.includes(".")) {
    return "Email";
  }
  if (text.includes("sk-") || text.includes("AIza")) {
    return "API Key";
  }
  const digitsOnly = text.replace(/\D/g, "");
  if (digitsOnly.length >= 9) {
    if (
      digitsOnly.startsWith("09") ||
      digitsOnly.startsWith("08") ||
      digitsOnly.startsWith("03") ||
      digitsOnly.startsWith("07") ||
      digitsOnly.startsWith("84")
    ) {
      return "Phone #";
    }
    return "ID / Number";
  }
  return null;
}

// ============================================================================
// Redaction Track Interpolation + Zero-Leak Padding
// ============================================================================

export interface InterpolatedBox {
  raw: NormalizedBox;
  padded: NormalizedBox;
  confidence: number;
}

function clamp01(v: number): number {
  return Math.max(0, Math.min(1, v));
}

export function expandBox(
  box: NormalizedBox,
  marginRatio: number
): NormalizedBox {
  const dw = box.w * marginRatio;
  const dh = box.h * marginRatio;
  const x1 = clamp01(box.x - dw * 0.5);
  const y1 = clamp01(box.y - dh * 0.5);
  const x2 = clamp01(box.x + box.w + dw * 0.5);
  const y2 = clamp01(box.y + box.h + dh * 0.5);
  return {
    x: x1,
    y: y1,
    w: Math.max(0, x2 - x1),
    h: Math.max(0, y2 - y1),
  };
}

export function trackFrameBounds(
  track: RedactionTrack,
  cfg: ShieldConfig,
  totalFrames: number
): [number, number, number, number] | null {
  if (track.keyframes.length === 0) return null;
  const first = track.keyframes[0].frame_index;
  const last = track.keyframes[track.keyframes.length - 1].frame_index;
  const pad = cfg.temporal_padding_frames;
  const padStart = Math.max(0, first - pad);
  const padEnd = Math.min(Math.max(totalFrames, last), last + pad);
  return [first, last, padStart, padEnd];
}

export function interpolateTrackAt(
  track: RedactionTrack,
  frameIndex: number,
  cfg: ShieldConfig
): InterpolatedBox | null {
  const kfs = track.keyframes;
  if (kfs.length === 0) return null;
  const pad = cfg.temporal_padding_frames;
  const first = kfs[0];
  const last = kfs[kfs.length - 1];

  if (frameIndex + pad < first.frame_index || frameIndex > last.frame_index + pad) {
    return null;
  }

  let raw: NormalizedBox;
  let confidence: number;

  if (frameIndex <= first.frame_index) {
    raw = first.bbox;
    confidence = first.confidence;
  } else if (frameIndex >= last.frame_index) {
    raw = last.bbox;
    confidence = last.confidence;
  } else {
    let lo = 0;
    let hi = kfs.length - 1;
    while (lo < hi) {
      const mid = (lo + hi) >>> 1;
      if (kfs[mid].frame_index < frameIndex) {
        lo = mid + 1;
      } else {
        hi = mid;
      }
    }
    if (kfs[lo].frame_index === frameIndex) {
      raw = kfs[lo].bbox;
      confidence = kfs[lo].confidence;
    } else {
      const prev = kfs[lo - 1];
      const next = kfs[lo];
      const span = Math.max(1, next.frame_index - prev.frame_index);
      const t = (frameIndex - prev.frame_index) / span;
      raw = {
        x: prev.bbox.x + (next.bbox.x - prev.bbox.x) * t,
        y: prev.bbox.y + (next.bbox.y - prev.bbox.y) * t,
        w: prev.bbox.w + (next.bbox.w - prev.bbox.w) * t,
        h: prev.bbox.h + (next.bbox.h - prev.bbox.h) * t,
      };
      confidence = prev.confidence + (next.confidence - prev.confidence) * t;
    }
  }

  return {
    raw,
    padded: expandBox(raw, cfg.spatial_padding_ratio),
    confidence,
  };
}

// ============================================================================
// Client-side Multi-Format Export Previewer (mirrors export-engine)
// ============================================================================

function formatSrtTs(ms: number): string {
  const safe = Math.max(0, Math.floor(ms));
  const h = Math.floor(safe / 3_600_000);
  let rem = safe % 3_600_000;
  const m = Math.floor(rem / 60_000);
  rem %= 60_000;
  const s = Math.floor(rem / 1_000);
  const mi = rem % 1_000;
  return `${pad2(h)}:${pad2(m)}:${pad2(s)},${pad3(mi)}`;
}

function formatAssTs(ms: number): string {
  const safe = Math.max(0, Math.floor(ms));
  const h = Math.floor(safe / 3_600_000);
  let rem = safe % 3_600_000;
  const m = Math.floor(rem / 60_000);
  rem %= 60_000;
  const s = Math.floor(rem / 1_000);
  const cs = Math.floor((rem % 1_000) / 10);
  return `${h}:${pad2(m)}:${pad2(s)}.${pad2(cs)}`;
}

function escapeXml(input: string): string {
  return input
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}

function wrapChars(text: string, maxChars: number): string {
  if (maxChars <= 0 || Array.from(text).length <= maxChars) {
    return text;
  }
  const lines: string[] = [];
  let current = "";
  for (const word of text.split(/\s+/)) {
    if (!word) continue;
    const nextLen = current
      ? Array.from(current).length + 1 + Array.from(word).length
      : Array.from(word).length;
    if (nextLen > maxChars && current) {
      lines.push(current);
      current = word;
    } else {
      current = current ? `${current} ${word}` : word;
    }
  }
  if (current) lines.push(current);
  return lines.join("\n");
}

export function prepareExportTranscript(
  transcript: Transcript,
  stripFillers: boolean,
  includeSpeaker: boolean,
  maxCharsPerLine: number
): Transcript {
  return {
    ...transcript,
    segments: transcript.segments.map((seg) => {
      let txt = stripFillers ? stripFillersText(seg.text) : seg.text.trim();
      txt = wrapChars(txt, maxCharsPerLine);
      if (includeSpeaker && seg.speaker && seg.speaker.trim().length > 0) {
        txt = `[${seg.speaker}] ${txt}`;
      }
      return { ...seg, text: txt };
    }),
  };
}

export function renderExportLocal(
  transcript: Transcript,
  media: MediaRef,
  format: string
): string {
  switch (format) {
    case "ass": {
      let out =
        "[Script Info]\nTitle: TinyCut Subtitles\nScriptType: v4.00+\nPlayResX: 1920\nPlayResY: 1080\n\n" +
        "[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, OutlineColour, BackColour, Bold, Alignment, MarginV\n" +
        "Style: Default,Be Vietnam Pro,54,&H00FFFFFF,&H00000000,&H64000000,0,2,48\n\n" +
        "[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n";
      for (const seg of transcript.segments) {
        const text = seg.text.trim().replace(/\n/g, "\\N");
        const sp = seg.speaker ?? "";
        out += `Dialogue: 0,${formatAssTs(seg.start_ms)},${formatAssTs(seg.end_ms)},Default,${sp},0,0,0,,${text}\n`;
      }
      return out;
    }
    case "fcpxml": {
      const w = Math.max(media.width, 1920);
      const h = Math.max(media.height, 1080);
      const fps = Math.max(24, Math.round(media.fps));
      const dur = Math.max(media.duration_ms, 1000);
      let out = `<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE fcpxml>\n<fcpxml version="1.10">\n  <resources>\n    <format id="r1" name="FFVideoFormat${h}p${fps}" frameDuration="1/${fps}s" width="${w}" height="${h}"/>\n  </resources>\n  <library>\n    <event name="TinyCut Export">\n      <project name="TinyCut Subtitles">\n        <sequence format="r1" duration="${dur}/1000s">\n          <spine>\n            <gap name="Subtitle Track" offset="0/1000s" duration="${dur}/1000s">\n`;
      transcript.segments.forEach((seg, i) => {
        const d = Math.max(1, seg.end_ms - seg.start_ms);
        out += `              <title lane="1" name="Sub ${i + 1}" offset="${seg.start_ms}/1000s" duration="${d}/1000s">\n                <text>${escapeXml(seg.text)}</text>\n              </title>\n`;
      });
      out +=
        "            </gap>\n          </spine>\n        </sequence>\n      </project>\n    </event>\n  </library>\n</fcpxml>\n";
      return out;
    }
    case "premiere_xml": {
      const tb = Math.max(24, Math.round(media.fps));
      const totalF = Math.round((media.duration_ms / 1000) * tb);
      let out = `<?xml version="1.0" encoding="UTF-8"?>\n<xmeml version="5">\n  <sequence>\n    <name>TinyCut Subtitles</name>\n    <duration>${totalF}</duration>\n    <rate><timebase>${tb}</timebase><ntsc>FALSE</ntsc></rate>\n`;
      transcript.segments.forEach((seg, i) => {
        const inf = Math.round((seg.start_ms / 1000) * tb);
        const outf = Math.round((seg.end_ms / 1000) * tb);
        out += `    <marker>\n      <name>Sub ${i + 1}</name>\n      <comment>${escapeXml(seg.text)}</comment>\n      <in>${inf}</in>\n      <out>${outf}</out>\n    </marker>\n`;
      });
      out += "  </sequence>\n</xmeml>\n";
      return out;
    }
    case "edl": {
      let out = "TITLE: TinyCut Subtitles\nFCM: NON-DROP FRAME\n\n";
      const fps = Math.max(24, media.fps);
      transcript.segments.forEach((seg, i) => {
        const s = formatSmpte(seg.start_ms, fps);
        const e = formatSmpte(seg.end_ms, fps);
        out += `${pad3(i + 1)}  AX       V     C        ${s} ${e} ${s} ${e}\n* FROM CLIP NAME: ${seg.text.replace(/\n/g, " ")}\n\n`;
      });
      return out;
    }
    default: {
      let out = "";
      transcript.segments.forEach((seg, i) => {
        out += `${i + 1}\n${formatSrtTs(seg.start_ms)} --> ${formatSrtTs(seg.end_ms)}\n${seg.text.trim()}\n\n`;
      });
      return out;
    }
  }
}

// ============================================================================
// Default Lightweight Models Catalog (browser demo fallback; live Tauri uses listModels)
// ============================================================================

export function defaultLightweightModels(): DefaultModelSpec[] {
  return [
    {
      task: "asr",
      id: "whisper-base-q5_1",
      display_name: "Whisper Base (Q5_1)",
      task_label: "Speech-to-Text (ASR)",
      filename: "ggml-base-q5_1.bin",
      runtime: "whisper-rs (GGML)",
      license: "MIT",
      download_url:
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base-q5_1.bin",
      sha256: "",
      size_bytes: 59_707_625,
      tier: "mobile",
    },
    {
      task: "vad",
      id: "silero-vad-v5",
      display_name: "Silero VAD v5",
      task_label: "Voice Activity Detection",
      filename: "silero_vad.onnx",
      runtime: "ort (ONNX)",
      license: "MIT",
      download_url:
        "https://github.com/snakers4/silero-vad/raw/master/src/silero_vad/data/silero_vad.onnx",
      sha256: "",
      size_bytes: 2_204_103,
      tier: "mobile",
    },
    {
      task: "text_fixer",
      id: "qwen2.5-0.5b-instruct-q4_k_m",
      display_name: "Qwen2.5 0.5B Instruct (Q4_K_M)",
      task_label: "Subtitle Fixer LLM",
      filename: "Qwen2.5-0.5B-Instruct-Q4_K_M.gguf",
      runtime: "ggml / glossary",
      license: "Apache-2.0",
      download_url:
        "https://huggingface.co/bartowski/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf",
      sha256: "",
      size_bytes: 397_808_192,
      tier: "mobile",
    },
    {
      task: "text_fixer_tokenizer",
      id: "qwen2.5-0.5b-tokenizer",
      display_name: "Qwen2.5 0.5B Tokenizer",
      task_label: "Subtitle Fixer Tokenizer",
      filename: "qwen2.5-0.5b-tokenizer.json",
      runtime: "tokenizers (JSON)",
      license: "Apache-2.0",
      download_url:
        "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct/resolve/main/tokenizer.json",
      sha256: "",
      size_bytes: 7_031_645,
      tier: "mobile",
    },
    {
      task: "face_detector",
      id: "yunet-2023mar",
      display_name: "YuNet 2023mar",
      task_label: "Face Detection",
      filename: "face_detection_yunet_2023mar.onnx",
      runtime: "ort (ONNX)",
      license: "MIT",
      download_url:
        "https://media.githubusercontent.com/media/opencv/opencv_zoo/main/models/face_detection_yunet/face_detection_yunet_2023mar.onnx",
      sha256: "",
      size_bytes: 232_589,
      tier: "mobile",
    },
    {
      task: "face_embedder",
      id: "sface-2021dec",
      display_name: "SFace 2021dec",
      task_label: "Creator Face Re-ID",
      filename: "face_recognition_sface_2021dec.onnx",
      runtime: "ort (ONNX)",
      license: "Apache-2.0",
      download_url:
        "https://media.githubusercontent.com/media/opencv/opencv_zoo/main/models/face_recognition_sface/face_recognition_sface_2021dec.onnx",
      sha256: "",
      size_bytes: 38_696_353,
      tier: "mobile",
    },
    {
      task: "plate_detector",
      id: "rtdetrv2-r18-plate",
      display_name: "RT-DETRv2 R18 Plate",
      task_label: "License Plate Detection",
      filename: "plate_rtdetr.onnx",
      runtime: "ort (ONNX)",
      license: "Apache-2.0",
      download_url:
        "https://huggingface.co/Topurrra/rtdetr-license-plate-detection-onnx/resolve/main/plate_rtdetr.onnx",
      sha256: "",
      size_bytes: 80_671_879,
      tier: "desktop",
    },
    {
      task: "screen_ocr_det",
      id: "ppocrv4-mobile-det",
      display_name: "PP-OCRv4 Mobile Det",
      task_label: "Screen PII Text Detect",
      filename: "ch_PP-OCRv4_det_infer.onnx",
      runtime: "ort (ONNX)",
      license: "Apache-2.0",
      download_url:
        "https://huggingface.co/SWHL/RapidOCR/resolve/main/PP-OCRv4/ch_PP-OCRv4_det_infer.onnx",
      sha256: "",
      size_bytes: 4_745_517,
      tier: "mobile",
    },
    {
      task: "screen_ocr_rec",
      id: "ppocrv3-mobile-en-rec",
      display_name: "PP-OCRv3 Mobile Rec",
      task_label: "Screen PII Text Read",
      filename: "en_PP-OCRv3_rec_infer.onnx",
      runtime: "ort (ONNX)",
      license: "Apache-2.0",
      download_url:
        "https://huggingface.co/SWHL/RapidOCR/resolve/main/PP-OCRv3/en_PP-OCRv3_rec_infer.onnx",
      sha256: "",
      size_bytes: 8_967_018,
      tier: "mobile",
    },
  ];
}
