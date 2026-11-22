use dioxus::prelude::*;
use serde::{Serialize, de::DeserializeOwned};
use tinycut_core_domain::{
    BlurStyle, GlossaryEntry, InferenceMode, MediaRef, NormalizedBox, RedactionKeyframe,
    RedactionKind, RedactionTrack, ShieldConfig, Transcript, TranscriptSegment,
};
use uuid::Uuid;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI_INTERNALS__"], js_name = invoke, catch)]
    async fn tauri_invoke_raw(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;
}

fn is_tauri_runtime() -> bool {
    js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("__TAURI_INTERNALS__"))
        .map(|v| !v.is_undefined() && !v.is_null())
        .unwrap_or(false)
}

async fn invoke_tauri<A: Serialize, R: DeserializeOwned>(cmd: &str, args: &A) -> Result<R, String> {
    let js_args = serde_wasm_bindgen::to_value(args).map_err(|e| e.to_string())?;
    let val = tauri_invoke_raw(cmd, js_args)
        .await
        .map_err(|e| format!("{e:?}"))?;
    serde_wasm_bindgen::from_value(val).map_err(|e| e.to_string())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActiveTab {
    Subtitles,
    PrivacyShield,
}

fn format_ms(ms: u64) -> String {
    let secs = ms / 1000;
    let mins = secs / 60;
    let rem_s = secs % 60;
    let millis = ms % 1000;
    format!("{mins:02}:{rem_s:02}.{millis:03}")
}

fn sample_media(path: &str) -> MediaRef {
    MediaRef {
        id: Uuid::new_v4(),
        path: path.to_string(),
        duration_ms: 12_000,
        width: 3840,
        height: 2160,
        fps: 60.0,
    }
}

fn sample_transcript() -> Transcript {
    Transcript {
        locale: "vi-VN".into(),
        asr_model_version: "whisper-v3-turbo-q5".into(),
        llm_model_version: Some("qwen3-1.7b-q4".into()),
        segments: vec![
            TranscriptSegment {
                id: Uuid::new_v4(),
                start_ms: 500,
                end_ms: 3_800,
                raw_asr: "ờ, hôm nay mình review cơ sơ trên mác búc".into(),
                llm_draft: Some("Hôm nay mình review Cursor trên MacBook.".into()),
                text: "Hôm nay mình review Cursor trên MacBook.".into(),
                speaker: Some("Host".into()),
                words: vec![],
            },
            TranscriptSegment {
                id: Uuid::new_v4(),
                start_ms: 4_100,
                end_ms: 8_200,
                raw_asr: "và tự động che biển số xe ngoài đường".into(),
                llm_draft: Some("Và tự động che biển số xe ngoài đường.".into()),
                text: "Và tự động che biển số xe ngoài đường.".into(),
                speaker: Some("Host".into()),
                words: vec![],
            },
        ],
    }
}

fn sample_tracks() -> Vec<RedactionTrack> {
    vec![
        RedactionTrack {
            track_id: 1,
            kind: RedactionKind::LicensePlate,
            enabled: true,
            whitelisted: false,
            keyframes: vec![
                RedactionKeyframe {
                    frame_index: 0,
                    bbox: NormalizedBox {
                        x: 0.35,
                        y: 0.68,
                        w: 0.16,
                        h: 0.08,
                    },
                    confidence: 0.94,
                },
                RedactionKeyframe {
                    frame_index: 120,
                    bbox: NormalizedBox {
                        x: 0.42,
                        y: 0.66,
                        w: 0.16,
                        h: 0.08,
                    },
                    confidence: 0.91,
                },
            ],
        },
        RedactionTrack {
            track_id: 2,
            kind: RedactionKind::Face,
            enabled: false,
            whitelisted: true,
            keyframes: vec![RedactionKeyframe {
                frame_index: 0,
                bbox: NormalizedBox {
                    x: 0.14,
                    y: 0.20,
                    w: 0.12,
                    h: 0.18,
                },
                confidence: 0.96,
            }],
        },
        RedactionTrack {
            track_id: 3,
            kind: RedactionKind::Face,
            enabled: true,
            whitelisted: false,
            keyframes: vec![RedactionKeyframe {
                frame_index: 15,
                bbox: NormalizedBox {
                    x: 0.72,
                    y: 0.24,
                    w: 0.10,
                    h: 0.15,
                },
                confidence: 0.88,
            }],
        },
    ]
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut mode = use_signal(|| InferenceMode::Local);
    let mut nda_private = use_signal(|| true);
    let mut active_tab = use_signal(|| ActiveTab::Subtitles);
    let mut media_path = use_signal(|| String::from("/videos/street_vlog_4k.mp4"));
    let mut media_ref = use_signal(|| sample_media("/videos/street_vlog_4k.mp4"));
    let mut transcript = use_signal(sample_transcript);
    let mut glossary = use_signal(|| {
        vec![
            GlossaryEntry {
                spoken_variant: "cơ sơ".into(),
                preferred_term: "Cursor".into(),
                hit_count: 4,
            },
            GlossaryEntry {
                spoken_variant: "mác búc".into(),
                preferred_term: "MacBook".into(),
                hit_count: 3,
            },
        ]
    });
    let mut spoken_input = use_signal(String::new);
    let mut preferred_input = use_signal(String::new);
    let mut export_preview = use_signal(String::new);
    let mut shield_cfg = use_signal(ShieldConfig::default);
    let mut tracks = use_signal(sample_tracks);
    let mut status = use_signal(|| String::from("Ready — 100% local & offline"));

    let on_run_subtitles = move |_| {
        let current_media = media_ref.read().clone();
        spawn(async move {
            status.set("Running local audio transcription & glossary cleanup...".into());
            if is_tauri_runtime() {
                #[derive(Serialize)]
                struct Args {
                    media: MediaRef,
                }
                match invoke_tauri::<_, Transcript>(
                    "transcribe_local",
                    &Args {
                        media: current_media,
                    },
                )
                .await
                {
                    Ok(t) => {
                        transcript.set(t);
                        status.set("Local transcription complete.".into());
                    }
                    Err(e) => status.set(format!("Transcription error: {e}")),
                }
            } else {
                transcript.set(sample_transcript());
                status.set("Preview mode: loaded sample bilingual transcript.".into());
            }
        });
    };

    let on_run_shield = move |_| {
        let current_media = media_ref.read().clone();
        let cfg = shield_cfg.read().clone();
        spawn(async move {
            status.set("Running local Visual Privacy Shield (ByteTrack + Zero-Leak)...".into());
            if is_tauri_runtime() {
                #[derive(Serialize)]
                struct Args {
                    media: MediaRef,
                    config: ShieldConfig,
                }
                match invoke_tauri::<_, Vec<RedactionTrack>>(
                    "detect_privacy_local",
                    &Args {
                        media: current_media,
                        config: cfg,
                    },
                )
                .await
                {
                    Ok(detected) => {
                        tracks.set(detected);
                        status.set("Privacy Shield scan complete.".into());
                    }
                    Err(e) => status.set(format!("Vision Shield error: {e}")),
                }
            } else {
                tracks.set(sample_tracks());
                status.set("Preview mode: loaded sample redaction tracks.".into());
            }
        });
    };

    let on_add_glossary = move |_| {
        let spoken = spoken_input.read().trim().to_string();
        let preferred = preferred_input.read().trim().to_string();
        if spoken.is_empty() || preferred.is_empty() {
            return;
        }
        spoken_input.set(String::new());
        preferred_input.set(String::new());

        spawn(async move {
            if is_tauri_runtime() {
                #[derive(Serialize)]
                struct Args {
                    spoken_variant: String,
                    preferred_term: String,
                }
                match invoke_tauri::<_, Vec<GlossaryEntry>>(
                    "upsert_glossary_term",
                    &Args {
                        spoken_variant: spoken,
                        preferred_term: preferred,
                    },
                )
                .await
                {
                    Ok(entries) => {
                        glossary.set(entries);
                        status.set("Saved term to local SQLite Personal Glossary.".into());
                    }
                    Err(e) => status.set(format!("Glossary error: {e}")),
                }
            } else {
                glossary.write().push(GlossaryEntry {
                    spoken_variant: spoken,
                    preferred_term: preferred,
                    hit_count: 1,
                });
                status.set("Added term to Personal Glossary (preview).".into());
            }
        });
    };

    let mut trigger_export = move |fmt: &'static str| {
        let current_t = transcript.read().clone();
        let current_m = media_ref.read().clone();
        spawn(async move {
            if is_tauri_runtime() {
                #[derive(Serialize)]
                struct Args {
                    transcript: Transcript,
                    media: MediaRef,
                    format: String,
                }
                match invoke_tauri::<_, String>(
                    "export_subtitles",
                    &Args {
                        transcript: current_t,
                        media: current_m,
                        format: fmt.to_string(),
                    },
                )
                .await
                {
                    Ok(text) => {
                        export_preview.set(text);
                        status.set(format!("Exported {fmt} successfully."));
                    }
                    Err(e) => status.set(format!("Export error: {e}")),
                }
            } else {
                let mut srt = String::new();
                for (i, s) in current_t.segments.iter().enumerate() {
                    srt.push_str(&format!(
                        "{}\n{} --> {}\n{}\n\n",
                        i + 1,
                        format_ms(s.start_ms),
                        format_ms(s.end_ms),
                        s.text
                    ));
                }
                export_preview.set(format!("[{fmt} preview]\n{srt}"));
                status.set(format!("Generated {fmt} preview."));
            }
        });
    };

    rsx! {
        div {
            style: "min-height: 100vh; background: #0f1117; color: #e6edf3; font-family: system-ui, -apple-system, sans-serif; padding: 20px;",
            div { style: "max-width: 1120px; margin: 0 auto;",
                // Header
                header { style: "display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid #262c36; padding-bottom: 14px; margin-bottom: 16px;",
                    div {
                        h1 { style: "margin: 0; font-size: 22px;", "TinyCut Studio" }
                        span { style: "font-size: 13px; color: #8b949e;", "Local-First Bilingual Subtitles & Visual Privacy Shield" }
                    }
                    div { style: "display: flex; gap: 10px; align-items: center;",
                        button {
                            style: if *mode.read() == InferenceMode::Local {
                                "padding: 6px 12px; border-radius: 6px; border: 1px solid #2f81f7; background: #1f6feb; color: #fff; cursor: pointer;"
                            } else {
                                "padding: 6px 12px; border-radius: 6px; border: 1px solid #30363d; background: #161b22; color: #c9d1d9; cursor: pointer;"
                            },
                            onclick: move |_| mode.set(InferenceMode::Local),
                            "Local (Free / Offline)"
                        }
                        button {
                            disabled: *nda_private.read(),
                            style: if *mode.read() == InferenceMode::CloudBoost {
                                "padding: 6px 12px; border-radius: 6px; border: 1px solid #a371f7; background: #8957e5; color: #fff; cursor: pointer;"
                            } else {
                                "padding: 6px 12px; border-radius: 6px; border: 1px solid #30363d; background: #161b22; color: #8b949e; cursor: pointer;"
                            },
                            onclick: move |_| mode.set(InferenceMode::CloudBoost),
                            "Cloud Boost (Pro)"
                        }
                        label { style: "display: flex; align-items: center; gap: 6px; font-size: 13px; color: #f0883e;",
                            input {
                                r#type: "checkbox",
                                checked: *nda_private.read(),
                                onchange: move |evt| {
                                    let locked = evt.checked();
                                    nda_private.set(locked);
                                    if locked {
                                        mode.set(InferenceMode::Local);
                                    }
                                }
                            }
                            "NDA Lock"
                        }
                    }
                }

                // Media Import Bar
                section { style: "display: flex; gap: 8px; margin-bottom: 14px; background: #161b22; padding: 12px; border-radius: 8px; border: 1px solid #262c36;",
                    input {
                        style: "flex: 1; padding: 8px 10px; border-radius: 6px; border: 1px solid #30363d; background: #0d1117; color: #e6edf3;",
                        value: "{media_path}",
                        oninput: move |evt| {
                            let p = evt.value();
                            media_path.set(p.clone());
                            media_ref.set(sample_media(&p));
                        },
                        placeholder: "Path to local video/audio file..."
                    }
                    button {
                        style: "padding: 8px 14px; border-radius: 6px; border: 1px solid #238636; background: #238636; color: #fff; cursor: pointer;",
                        onclick: on_run_subtitles,
                        "Transcribe & Fix"
                    }
                    button {
                        style: "padding: 8px 14px; border-radius: 6px; border: 1px solid #1f6feb; background: #1f6feb; color: #fff; cursor: pointer;",
                        onclick: on_run_shield,
                        "Scan Privacy Shield"
                    }
                }

                // Status bar & Tabs
                div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 14px;",
                    div { style: "display: flex; gap: 8px;",
                        button {
                            style: if *active_tab.read() == ActiveTab::Subtitles {
                                "padding: 8px 14px; border-radius: 6px; border: none; background: #21262d; color: #fff; font-weight: 600; cursor: pointer;"
                            } else {
                                "padding: 8px 14px; border-radius: 6px; border: none; background: transparent; color: #8b949e; cursor: pointer;"
                            },
                            onclick: move |_| active_tab.set(ActiveTab::Subtitles),
                            "Subtitles & Personal Glossary"
                        }
                        button {
                            style: if *active_tab.read() == ActiveTab::PrivacyShield {
                                "padding: 8px 14px; border-radius: 6px; border: none; background: #21262d; color: #fff; font-weight: 600; cursor: pointer;"
                            } else {
                                "padding: 8px 14px; border-radius: 6px; border: none; background: transparent; color: #8b949e; cursor: pointer;"
                            },
                            onclick: move |_| active_tab.set(ActiveTab::PrivacyShield),
                            "Visual Privacy Shield"
                        }
                    }
                    span { style: "font-size: 12px; color: #7ee787;", "{status}" }
                }

                if *active_tab.read() == ActiveTab::Subtitles {
                    div { style: "display: grid; grid-template-columns: 2fr 1fr; gap: 16px;",
                        // Subtitle Diff Editor
                        section { style: "background: #161b22; border: 1px solid #262c36; border-radius: 8px; padding: 14px;",
                            h2 { style: "margin: 0 0 12px 0; font-size: 16px;", "Interactive Subtitle & Diff Editor" }
                            for (idx, seg) in transcript.read().segments.iter().cloned().enumerate() {
                                div {
                                    key: "{seg.id}",
                                    style: "padding: 10px; margin-bottom: 10px; background: #0d1117; border: 1px solid #30363d; border-radius: 6px;",
                                    div { style: "display: flex; justify-content: space-between; font-size: 12px; color: #8b949e; margin-bottom: 6px;",
                                        span { "{format_ms(seg.start_ms)} -> {format_ms(seg.end_ms)}" }
                                        span { "ASR: {seg.raw_asr}" }
                                    }
                                    input {
                                        style: "width: 100%; box-sizing: border-box; padding: 8px; border-radius: 4px; border: 1px solid #30363d; background: #161b22; color: #e6edf3;",
                                        value: "{seg.text}",
                                        oninput: move |evt| {
                                            if let Some(target) = transcript.write().segments.get_mut(idx) {
                                                target.text = evt.value();
                                            }
                                        }
                                    }
                                }
                            }

                            div { style: "display: flex; gap: 8px; flex-wrap: wrap; margin-top: 12px;",
                                button { onclick: move |_| trigger_export("srt"), style: "padding: 6px 10px; border-radius: 6px; border: 1px solid #30363d; background: #21262d; color: #e6edf3; cursor: pointer;", "Export SRT" }
                                button { onclick: move |_| trigger_export("ass"), style: "padding: 6px 10px; border-radius: 6px; border: 1px solid #30363d; background: #21262d; color: #e6edf3; cursor: pointer;", "Export ASS" }
                                button { onclick: move |_| trigger_export("fcpxml"), style: "padding: 6px 10px; border-radius: 6px; border: 1px solid #30363d; background: #21262d; color: #e6edf3; cursor: pointer;", "Export FCPXML" }
                                button { onclick: move |_| trigger_export("premiere_xml"), style: "padding: 6px 10px; border-radius: 6px; border: 1px solid #30363d; background: #21262d; color: #e6edf3; cursor: pointer;", "Premiere XML" }
                                button { onclick: move |_| trigger_export("edl"), style: "padding: 6px 10px; border-radius: 6px; border: 1px solid #30363d; background: #21262d; color: #e6edf3; cursor: pointer;", "DaVinci EDL" }
                            }

                            if !export_preview.read().is_empty() {
                                textarea {
                                    readonly: true,
                                    rows: "6",
                                    style: "width: 100%; box-sizing: border-box; margin-top: 10px; padding: 8px; border-radius: 6px; border: 1px solid #30363d; background: #0d1117; color: #79c0ff; font-family: monospace; font-size: 12px;",
                                    value: "{export_preview}"
                                }
                            }
                        }

                        // Personal Glossary Panel
                        aside { style: "background: #161b22; border: 1px solid #262c36; border-radius: 8px; padding: 14px;",
                            h2 { style: "margin: 0 0 10px 0; font-size: 16px;", "Personal Glossary (SQLite)" }
                            div { style: "display: flex; flex-direction: column; gap: 6px; margin-bottom: 12px;",
                                input {
                                    style: "padding: 7px; border-radius: 6px; border: 1px solid #30363d; background: #0d1117; color: #e6edf3;",
                                    placeholder: "Spoken (e.g. cơ sơ)",
                                    value: "{spoken_input}",
                                    oninput: move |evt| spoken_input.set(evt.value())
                                }
                                input {
                                    style: "padding: 7px; border-radius: 6px; border: 1px solid #30363d; background: #0d1117; color: #e6edf3;",
                                    placeholder: "Preferred (e.g. Cursor)",
                                    value: "{preferred_input}",
                                    oninput: move |evt| preferred_input.set(evt.value())
                                }
                                button {
                                    style: "padding: 7px; border-radius: 6px; border: 1px solid #238636; background: #238636; color: #fff; cursor: pointer;",
                                    onclick: on_add_glossary,
                                    "Save to Glossary"
                                }
                            }
                            for entry in glossary.read().iter() {
                                div { style: "display: flex; justify-content: space-between; padding: 6px 8px; margin-bottom: 6px; background: #0d1117; border-radius: 6px; font-size: 13px;",
                                    span { "{entry.spoken_variant} -> {entry.preferred_term}" }
                                    span { style: "color: #8b949e;", "x{entry.hit_count}" }
                                }
                            }
                        }
                    }
                } else {
                    // Visual Privacy Shield Panel
                    section { style: "background: #161b22; border: 1px solid #262c36; border-radius: 8px; padding: 14px;",
                        div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; flex-wrap: wrap; gap: 10px;",
                            h2 { style: "margin: 0; font-size: 16px;", "Detected Redaction Tracks (ByteTrack + Zero-Leak)" }
                            div { style: "display: flex; gap: 12px; font-size: 13px;",
                                label {
                                    input {
                                        r#type: "checkbox",
                                        checked: shield_cfg.read().detect_plates,
                                        onchange: move |e| shield_cfg.write().detect_plates = e.checked()
                                    }
                                    " License Plates"
                                }
                                label {
                                    input {
                                        r#type: "checkbox",
                                        checked: shield_cfg.read().detect_faces,
                                        onchange: move |e| shield_cfg.write().detect_faces = e.checked()
                                    }
                                    " Faces"
                                }
                                button {
                                    style: "padding: 4px 8px; border-radius: 4px; border: 1px solid #30363d; background: #21262d; color: #e6edf3; cursor: pointer;",
                                    onclick: move |_| {
                                        let next = match shield_cfg.read().blur_style {
                                            BlurStyle::Gaussian => BlurStyle::Pixelate,
                                            BlurStyle::Pixelate => BlurStyle::SolidMask,
                                            BlurStyle::SolidMask => BlurStyle::Gaussian,
                                        };
                                        shield_cfg.write().blur_style = next;
                                    },
                                    "Style: {shield_cfg.read().blur_style:?}"
                                }
                            }
                        }

                        for (idx, track) in tracks.read().iter().cloned().enumerate() {
                            div {
                                key: "{track.track_id}",
                                style: "display: flex; justify-content: space-between; align-items: center; padding: 10px; margin-bottom: 8px; background: #0d1117; border: 1px solid #30363d; border-radius: 6px;",
                                div {
                                    strong { "Track #{track.track_id} — {track.kind:?}" }
                                    div { style: "font-size: 12px; color: #8b949e;",
                                        "Keyframes: {track.keyframes.len()} | Zero-Leak: +{shield_cfg.read().spatial_padding_ratio * 100.0:.0}% / +/-{shield_cfg.read().temporal_padding_frames}f"
                                    }
                                }
                                div { style: "display: flex; gap: 8px; align-items: center;",
                                    label { style: "font-size: 13px;",
                                        input {
                                            r#type: "checkbox",
                                            checked: track.enabled,
                                            onchange: move |evt| {
                                                if let Some(t) = tracks.write().get_mut(idx) {
                                                    t.enabled = evt.checked();
                                                }
                                            }
                                        }
                                        " Blur Active"
                                    }
                                    if track.kind == RedactionKind::Face {
                                        button {
                                            style: if track.whitelisted {
                                                "padding: 4px 10px; border-radius: 6px; border: 1px solid #238636; background: #238636; color: #fff; cursor: pointer;"
                                            } else {
                                                "padding: 4px 10px; border-radius: 6px; border: 1px solid #30363d; background: #21262d; color: #c9d1d9; cursor: pointer;"
                                            },
                                            onclick: move |_| {
                                                if let Some(t) = tracks.write().get_mut(idx) {
                                                    t.whitelisted = !t.whitelisted;
                                                    t.enabled = !t.whitelisted;
                                                }
                                            },
                                            if track.whitelisted { "Creator Whitelisted" } else { "Keep Unblurred" }
                                        }
                                    }
                                    button {
                                        style: "padding: 4px 10px; border-radius: 6px; border: 1px solid #6e7681; background: transparent; color: #8b949e; cursor: pointer;",
                                        onclick: move |_| {
                                            if let Some(t) = tracks.write().get_mut(idx) {
                                                t.enabled = false;
                                            }
                                            status.set(format!("Marked Track #{} as Not PII (false positive).", track.track_id));
                                        },
                                        "Not PII"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
