use crate::bridge::{cmd_detect_privacy_local, cmd_whitelist_creator_face, is_tauri_runtime};
use crate::domain_ext::{format_seg_time, frame_to_ms};
use crate::state::{StudioState, ToastSeverity, TrackVerdict, sample_tracks};
use dioxus::prelude::*;
use tinycut_core_domain::{BlurStyle, RedactionKind};

#[component]
pub fn ShieldInspector() -> Element {
    let mut state = use_context::<StudioState>();

    let cfg = state.shield_cfg.read().clone();
    let tracks = state.tracks.read().clone();
    let verdicts = state.track_verdicts.read().clone();
    let selected_id = *state.selected_track_id.read();
    let fps = state.project.read().media.fps.max(1.0);
    let busy = state.busy_job.read().is_some();

    let plate_count = tracks.iter().filter(|t| t.kind == RedactionKind::LicensePlate).count();
    let face_count = tracks.iter().filter(|t| t.kind == RedactionKind::Face).count();
    let pii_count = tracks.iter().filter(|t| t.kind == RedactionKind::ScreenPii).count();
    let qr_count = tracks.iter().filter(|t| t.kind == RedactionKind::QrCode).count();

    let on_scan = move |_| {
        let media = state.project.read().media.clone();
        let scan_cfg = state.shield_cfg.read().clone();
        state.busy_job.set(Some("Privacy Shield Scan".into()));
        spawn(async move {
            if is_tauri_runtime() {
                match cmd_detect_privacy_local(media, scan_cfg).await {
                    Ok(detected) => {
                        state.push_undo();
                        let n = detected.len();
                        state.tracks.set(detected);
                        state.notify(
                            ToastSeverity::Success,
                            format!("Detected {n} privacy track(s)"),
                        );
                    }
                    Err(e) => {
                        state.notify(ToastSeverity::Error, format!("Shield scan failed: {e}"));
                    }
                }
            } else {
                state.push_undo();
                state.tracks.set(sample_tracks());
                state.notify(
                    ToastSeverity::Success,
                    "Preview: ByteTrack + Zero-Leak updated 4 tracks",
                );
            }
            state.busy_job.set(None);
        });
    };

    rsx! {
        div { class: "tc-insp-header",
            div { class: "tc-insp-header__title",
                span { "Privacy Shield" }
                span { class: "tc-insp-header__count tc-mono", "{tracks.len()}" }
            }
            button {
                class: "tc-btn tc-btn--sm",
                disabled: busy,
                onclick: on_scan,
                if busy { "Scanning…" } else { "Detect" }
            }
        }

        div { class: "tc-insp-body",
            // macOS Inset Grouped Form Controls
            div { class: "tc-insp-body--form",
                // 1. Detectors Group
                div { class: "tc-group",
                    div { class: "tc-group__title",
                        span { "Detectors" }
                        span { class: "tc-group__sub", "Local ONNX" }
                    }
                    div { class: "tc-group__card",
                        div { class: "tc-form-row",
                            div { class: "tc-form-row__label",
                                span { class: "tc-swatch tc-swatch--plate" }
                                span { "License plates" }
                                span { class: "tc-form-row__sub tc-mono", "{plate_count}" }
                            }
                            button {
                                class: if cfg.detect_plates { "tc-switch tc-switch--on" } else { "tc-switch" },
                                role: "switch",
                                "aria-checked": "{cfg.detect_plates}",
                                onclick: move |_| {
                                    let v = !state.shield_cfg.read().detect_plates;
                                    state.shield_cfg.write().detect_plates = v;
                                },
                            }
                        }
                        div { class: "tc-form-row",
                            div { class: "tc-form-row__label",
                                span { class: "tc-swatch tc-swatch--face" }
                                span { "Faces" }
                                span { class: "tc-form-row__sub tc-mono", "{face_count}" }
                            }
                            button {
                                class: if cfg.detect_faces { "tc-switch tc-switch--on" } else { "tc-switch" },
                                role: "switch",
                                "aria-checked": "{cfg.detect_faces}",
                                onclick: move |_| {
                                    let v = !state.shield_cfg.read().detect_faces;
                                    state.shield_cfg.write().detect_faces = v;
                                },
                            }
                        }
                        div { class: "tc-form-row",
                            div { class: "tc-form-row__label",
                                span { class: "tc-swatch tc-swatch--pii" }
                                span { "Screen PII" }
                                span { class: "tc-form-row__sub tc-mono", "{pii_count}" }
                            }
                            button {
                                class: if cfg.detect_screen_pii { "tc-switch tc-switch--on" } else { "tc-switch" },
                                role: "switch",
                                "aria-checked": "{cfg.detect_screen_pii}",
                                onclick: move |_| {
                                    let v = !state.shield_cfg.read().detect_screen_pii;
                                    state.shield_cfg.write().detect_screen_pii = v;
                                },
                            }
                        }
                        div { class: "tc-form-row",
                            div { class: "tc-form-row__label",
                                span { class: "tc-swatch tc-swatch--pii" }
                                span { "QR codes" }
                                span { class: "tc-form-row__sub tc-mono", "{qr_count}" }
                            }
                            button {
                                class: if cfg.detect_qr { "tc-switch tc-switch--on" } else { "tc-switch" },
                                role: "switch",
                                "aria-checked": "{cfg.detect_qr}",
                                onclick: move |_| {
                                    let v = !state.shield_cfg.read().detect_qr;
                                    state.shield_cfg.write().detect_qr = v;
                                },
                            }
                        }
                    }
                }

                // 2. Mask Style & Zero-Leak Padding Group
                div { class: "tc-group",
                    div { class: "tc-group__title",
                        span { "Redaction & Zero-Leak" }
                        span { class: "tc-group__sub", "wgpu shader" }
                    }
                    div { class: "tc-group__card",
                        div { class: "tc-form-row tc-form-row--stack",
                            div { class: "tc-seg",
                                for (style_opt, label) in [
                                    (BlurStyle::Gaussian, "Gaussian"),
                                    (BlurStyle::Pixelate, "Pixelate"),
                                    (BlurStyle::SolidMask, "Solid"),
                                ] {
                                    button {
                                        key: "{label}",
                                        class: if cfg.blur_style == style_opt {
                                            "tc-seg__btn tc-seg__btn--active"
                                        } else {
                                            "tc-seg__btn"
                                        },
                                        onclick: move |_| state.shield_cfg.write().blur_style = style_opt,
                                        "{label}"
                                    }
                                }
                            }
                        }
                        div { class: "tc-form-row",
                            span { class: "tc-form-row__label", "Spatial box margin" }
                            div { class: "tc-stepper",
                                button {
                                    class: "tc-stepper__btn",
                                    onclick: move |_| {
                                        let cur = state.shield_cfg.read().spatial_padding_ratio;
                                        state.shield_cfg.write().spatial_padding_ratio = (cur - 0.05).max(0.0);
                                    },
                                    "−"
                                }
                                span { class: "tc-stepper__val tc-mono", "+{cfg.spatial_padding_ratio * 100.0:.0}%" }
                                button {
                                    class: "tc-stepper__btn",
                                    onclick: move |_| {
                                        let cur = state.shield_cfg.read().spatial_padding_ratio;
                                        state.shield_cfg.write().spatial_padding_ratio = (cur + 0.05).min(0.30);
                                    },
                                    "+"
                                }
                            }
                        }
                        div { class: "tc-form-row",
                            span { class: "tc-form-row__label", "Temporal lead / trail" }
                            div { class: "tc-stepper",
                                button {
                                    class: "tc-stepper__btn",
                                    onclick: move |_| {
                                        let cur = state.shield_cfg.read().temporal_padding_frames;
                                        state.shield_cfg.write().temporal_padding_frames = cur.saturating_sub(1);
                                    },
                                    "−"
                                }
                                span { class: "tc-stepper__val tc-mono", "±{cfg.temporal_padding_frames}f" }
                                button {
                                    class: "tc-stepper__btn",
                                    onclick: move |_| {
                                        let cur = state.shield_cfg.read().temporal_padding_frames;
                                        state.shield_cfg.write().temporal_padding_frames = (cur + 1).min(15);
                                    },
                                    "+"
                                }
                            }
                        }
                    }
                }

                div { class: "tc-group__title",
                    span { "Tracked Objects" }
                    span { class: "tc-group__sub", "Click row to seek" }
                }
            }

            // 3. Flat Track List Rows (matching Subtitles list pattern)
            div { class: "tc-insp-body--list",
                for (idx, track) in tracks.iter().cloned().enumerate() {
                    {
                        let tid = track.track_id;
                        let is_selected = selected_id == Some(tid);
                        let verdict = verdicts.get(&tid).copied();
                        let is_rejected = verdict == Some(TrackVerdict::NotPii);
                        let first_ms = track
                            .keyframes
                            .first()
                            .map(|k| frame_to_ms(k.frame_index, fps))
                            .unwrap_or(0);
                        let last_ms = track
                            .keyframes
                            .last()
                            .map(|k| frame_to_ms(k.frame_index, fps))
                            .unwrap_or(first_ms);
                        let conf_pct = track
                            .keyframes
                            .first()
                            .map(|k| (k.confidence * 100.0).round() as u32)
                            .unwrap_or(90);

                        let swatch_cls = if is_rejected {
                            "tc-swatch tc-swatch--muted"
                        } else if track.whitelisted {
                            "tc-swatch tc-swatch--wl"
                        } else {
                            match track.kind {
                                RedactionKind::LicensePlate => "tc-swatch tc-swatch--plate",
                                RedactionKind::Face => "tc-swatch tc-swatch--face",
                                RedactionKind::ScreenPii | RedactionKind::QrCode => "tc-swatch tc-swatch--pii",
                            }
                        };

                        let kind_title = match track.kind {
                            RedactionKind::LicensePlate => "License Plate",
                            RedactionKind::Face if track.whitelisted => "Creator Face",
                            RedactionKind::Face => "Bystander Face",
                            RedactionKind::ScreenPii => "Screen PII",
                            RedactionKind::QrCode => "QR Code",
                        };

                        let is_blurring = track.enabled && !track.whitelisted && !is_rejected;

                        rsx! {
                            div {
                                key: "trk-row-{tid}",
                                class: if is_selected { "tc-row tc-row--active" } else { "tc-row" },
                                onclick: move |_| {
                                    state.selected_track_id.set(Some(tid));
                                    state.seek_ms(first_ms);
                                },

                                div { class: "tc-track-row",
                                    div { class: "tc-track-row__top",
                                        div { class: "tc-track-row__title",
                                            span { class: "{swatch_cls}" }
                                            span { "{kind_title} #{tid}" }
                                            span { class: "tc-track-row__meta tc-mono", "{conf_pct}%" }
                                        }
                                        span { class: "tc-track-row__meta tc-mono",
                                            "{format_seg_time(first_ms)} – {format_seg_time(last_ms)}"
                                        }
                                    }

                                    if is_selected {
                                        div { class: "tc-track-row__controls",
                                            button {
                                                class: if is_blurring { "tc-btn tc-btn--sm tc-btn--active" } else { "tc-btn tc-btn--sm" },
                                                onclick: move |evt| {
                                                    evt.stop_propagation();
                                                    state.push_undo();
                                                    if let Some(t) = state.tracks.write().get_mut(idx) {
                                                        t.enabled = !t.enabled;
                                                        if t.enabled {
                                                            t.whitelisted = false;
                                                            state.track_verdicts.write().remove(&tid);
                                                        }
                                                    }
                                                },
                                                if is_blurring { "Blurring" } else { "Enable blur" }
                                            }

                                            if track.kind == RedactionKind::Face {
                                                button {
                                                    class: if track.whitelisted { "tc-btn tc-btn--sm tc-btn--active" } else { "tc-btn tc-btn--sm" },
                                                    onclick: move |evt| {
                                                        evt.stop_propagation();
                                                        state.push_undo();
                                                        let mut becoming_wl = false;
                                                        if let Some(t) = state.tracks.write().get_mut(idx) {
                                                            t.whitelisted = !t.whitelisted;
                                                            t.enabled = !t.whitelisted;
                                                            becoming_wl = t.whitelisted;
                                                        }
                                                        if becoming_wl {
                                                            state.track_verdicts.write().insert(tid, TrackVerdict::Whitelisted);
                                                            state.notify(ToastSeverity::Success, format!("Saved Face #{tid} to Creator Whitelist"));
                                                            spawn(async move {
                                                                if is_tauri_runtime() {
                                                                    let _ = cmd_whitelist_creator_face(
                                                                        format!("creator-{tid}"),
                                                                        "Host".into(),
                                                                        vec![0.6, 0.8],
                                                                    ).await;
                                                                }
                                                            });
                                                        } else {
                                                            state.track_verdicts.write().remove(&tid);
                                                            state.notify(ToastSeverity::Info, format!("Un-whitelisted Face #{tid}"));
                                                        }
                                                    },
                                                    if track.whitelisted { "Whitelisted" } else { "Keep Creator" }
                                                }
                                            }

                                            button {
                                                class: if is_rejected { "tc-btn tc-btn--sm tc-btn--danger" } else { "tc-btn tc-btn--ghost tc-btn--sm" },
                                                onclick: move |evt| {
                                                    evt.stop_propagation();
                                                    state.push_undo();
                                                    if is_rejected {
                                                        state.track_verdicts.write().remove(&tid);
                                                        if let Some(t) = state.tracks.write().get_mut(idx) {
                                                            t.enabled = true;
                                                        }
                                                        state.notify(ToastSeverity::Info, format!("Restored Track #{tid}"));
                                                    } else {
                                                        state.track_verdicts.write().insert(tid, TrackVerdict::NotPii);
                                                        if let Some(t) = state.tracks.write().get_mut(idx) {
                                                            t.enabled = false;
                                                            t.whitelisted = false;
                                                        }
                                                        state.notify(ToastSeverity::Info, format!("Marked Track #{tid} Not PII"));
                                                    }
                                                },
                                                if is_rejected { "Restore" } else { "Not PII" }
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
    }
}
