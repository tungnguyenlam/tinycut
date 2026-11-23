use crate::bridge::{cmd_whitelist_creator_face, convert_file_src, is_tauri_runtime};
use crate::components::icons::{
    IconChevronDown, IconLock, IconPause, IconPlay, IconSkipBack, IconSliders, IconStepBack,
    IconStepForward,
};
use crate::domain_ext::{format_smpte, interpolate_track_at, ms_to_frame};
use crate::state::{StudioMode, StudioState, ToastSeverity, TrackVerdict};
use dioxus::prelude::*;
use tinycut_core_domain::{BlurStyle, RedactionKind};

#[component]
pub fn Stage() -> Element {
    let mut state = use_context::<StudioState>();

    let project = state.project.read().clone();
    let is_demo = *state.is_demo_media.read();
    let current_ms = *state.current_ms.read();
    let playing = *state.playing.read();
    let rate = *state.playback_rate.read();
    let fps = project.media.fps.max(1.0);
    let current_frame = ms_to_frame(current_ms, fps);

    let cfg = state.shield_cfg.read().clone();
    let show_masks = *state.show_masks.read();
    let show_margin = *state.show_zero_leak_margin.read();
    let show_subs = *state.show_subtitles_overlay.read();
    let show_view_menu = *state.show_view_menu.read();
    let selected_track = *state.selected_track_id.read();
    let verdicts = state.track_verdicts.read().clone();
    let tracks = state.tracks.read().clone();

    let active_seg = state
        .transcript
        .read()
        .segments
        .iter()
        .find(|s| current_ms >= s.start_ms && current_ms <= s.end_ms)
        .cloned();
    let is_active_beeped = active_seg
        .as_ref()
        .map(|s| state.beeped_segments.read().contains(&s.id))
        .unwrap_or(false);

    let video_src = if !is_demo {
        convert_file_src(&project.media.path)
    } else {
        None
    };

    let hud_cls = if show_view_menu {
        "tc-stage-hud tc-stage-hud--pinned"
    } else {
        "tc-stage-hud tc-stage-hud--hoverable"
    };

    rsx! {
        section { class: "tc-stage-wrap",
            div { class: "tc-stage-canvas-area",
                // Hover-revealed Minimal Stage HUD (Demo / NDA left, View menu right)
                div { class: "{hud_cls}",
                    div { class: "tc-stage-hud__group",
                        if is_demo {
                            span { class: "tc-hud-chip", "Demo" }
                        }
                        if project.is_nda_private {
                            span { class: "tc-hud-chip tc-hud-chip--nda",
                                IconLock {}
                                "NDA"
                            }
                        }
                    }

                    div { class: "tc-stage-hud__group",
                        button {
                            class: "tc-hud-chip tc-hud-chip--btn",
                            title: "Viewer overlay options",
                            onclick: move |_| {
                                let next = !*state.show_view_menu.read();
                                state.close_overlays();
                                state.show_view_menu.set(next);
                            },
                            IconSliders {}
                            "View"
                            IconChevronDown {}
                        }
                    }
                }

                // Hover/Click View Options Glass Popover
                if show_view_menu {
                    div {
                        class: "tc-click-away",
                        onclick: move |_| state.show_view_menu.set(false),
                    }
                    div { class: "tc-popover tc-popover--view tc-glass-thick",
                        div { class: "tc-popover__heading", "Stage Overlays" }
                        div { class: "tc-form-row",
                            span { class: "tc-form-row__label", "Live blur masks" }
                            button {
                                class: if show_masks { "tc-switch tc-switch--on" } else { "tc-switch" },
                                role: "switch",
                                "aria-checked": "{show_masks}",
                                onclick: move |_| state.show_masks.set(!show_masks),
                            }
                        }
                        div { class: "tc-form-row",
                            span { class: "tc-form-row__label", "Zero-Leak margin" }
                            button {
                                class: if show_margin { "tc-switch tc-switch--on" } else { "tc-switch" },
                                role: "switch",
                                "aria-checked": "{show_margin}",
                                onclick: move |_| state.show_zero_leak_margin.set(!show_margin),
                            }
                        }
                        div { class: "tc-form-row",
                            span { class: "tc-form-row__label", "Subtitle preview" }
                            button {
                                class: if show_subs { "tc-switch tc-switch--on" } else { "tc-switch" },
                                role: "switch",
                                "aria-checked": "{show_subs}",
                                onclick: move |_| state.show_subtitles_overlay.set(!show_subs),
                            }
                        }
                    }
                }

                // 16:9 Cinema Frame (never overlapped by transport)
                div { class: "tc-stage-viewport",
                    if let Some(src) = video_src {
                        video {
                            class: "tc-video-el",
                            src: "{src}",
                            controls: false,
                            muted: false,
                        }
                    } else {
                        div { class: "tc-demo-scene",
                            div { class: "tc-demo-scene__sky-glow" }
                            div { class: "tc-demo-scene__city" }
                            div { class: "tc-demo-scene__road",
                                div { class: "tc-demo-scene__lane-dash" }
                            }

                            for track in tracks.iter() {
                                if let Some(interp) = interpolate_track_at(track, current_frame, &cfg) {
                                    {
                                        let r = interp.raw;
                                        let style_pos = format!(
                                            "left: {:.2}%; top: {:.2}%; width: {:.2}%; height: {:.2}%;",
                                            r.x * 100.0,
                                            r.y * 100.0,
                                            r.w * 100.0,
                                            r.h * 100.0
                                        );
                                        match track.kind {
                                            RedactionKind::LicensePlate => rsx! {
                                                div {
                                                    key: "subj-{track.track_id}",
                                                    class: "tc-demo-subject tc-demo-subject--plate",
                                                    style: "{style_pos}",
                                                    span { "59-X3" }
                                                    span { "829.41" }
                                                }
                                            },
                                            RedactionKind::Face if track.whitelisted => rsx! {
                                                div {
                                                    key: "subj-{track.track_id}",
                                                    class: "tc-demo-subject tc-demo-subject--creator",
                                                    style: "{style_pos}",
                                                    span { class: "tc-demo-subject__cap", "HOST" }
                                                }
                                            },
                                            RedactionKind::Face => rsx! {
                                                div {
                                                    key: "subj-{track.track_id}",
                                                    class: "tc-demo-subject tc-demo-subject--bystander",
                                                    style: "{style_pos}",
                                                    span { class: "tc-demo-subject__cap", "PASSERBY" }
                                                }
                                            },
                                            RedactionKind::ScreenPii | RedactionKind::QrCode => rsx! {
                                                div {
                                                    key: "subj-{track.track_id}",
                                                    class: "tc-demo-subject tc-demo-subject--pii",
                                                    style: "{style_pos}",
                                                    span { class: "tc-demo-subject__sub", "Notification · Zalo" }
                                                    span { "OTP: 482910 · minh@tinycut.dev" }
                                                }
                                            },
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Interactive Redaction Overlay Layer (crisp 2px detection rectangles)
                    div { class: "tc-redaction-layer",
                        for (idx, track) in tracks.iter().cloned().enumerate() {
                            if let Some(interp) = interpolate_track_at(&track, current_frame, &cfg) {
                                {
                                    let p = interp.padded;
                                    let r = interp.raw;
                                    let verdict = verdicts.get(&track.track_id).copied();
                                    let is_selected = selected_track == Some(track.track_id);
                                    let is_rejected = verdict == Some(TrackVerdict::NotPii);

                                    let (box_kind, tag_kind) = if is_rejected || (!track.enabled && !track.whitelisted) {
                                        ("tc-rbox--disabled", "tc-rbox__tag--disabled")
                                    } else if track.whitelisted {
                                        ("tc-rbox--whitelist", "tc-rbox__tag--whitelist")
                                    } else {
                                        match track.kind {
                                            RedactionKind::LicensePlate => ("tc-rbox--plate", "tc-rbox__tag--plate"),
                                            RedactionKind::Face => ("tc-rbox--face", "tc-rbox__tag--face"),
                                            RedactionKind::ScreenPii | RedactionKind::QrCode => ("tc-rbox--pii", "tc-rbox__tag--pii"),
                                        }
                                    };

                                    let box_class = if is_selected {
                                        format!("tc-rbox {box_kind} tc-rbox--selected")
                                    } else {
                                        format!("tc-rbox {box_kind}")
                                    };

                                    let mask_class = match cfg.blur_style {
                                        BlurStyle::Gaussian => "tc-rbox__mask tc-rbox__mask--gaussian",
                                        BlurStyle::Pixelate => "tc-rbox__mask tc-rbox__mask--pixelate",
                                        BlurStyle::SolidMask => "tc-rbox__mask tc-rbox__mask--solid",
                                    };

                                    let kind_label = match track.kind {
                                        RedactionKind::LicensePlate => "Plate",
                                        RedactionKind::Face if track.whitelisted => "Creator",
                                        RedactionKind::Face => "Face",
                                        RedactionKind::ScreenPii => "Screen PII",
                                        RedactionKind::QrCode => "QR",
                                    };

                                    let box_style = format!(
                                        "left: {:.2}%; top: {:.2}%; width: {:.2}%; height: {:.2}%;",
                                        p.x * 100.0,
                                        p.y * 100.0,
                                        p.w * 100.0,
                                        p.h * 100.0
                                    );

                                    let inner_left = if p.w > 0.0 { ((r.x - p.x) / p.w) * 100.0 } else { 0.0 };
                                    let inner_top = if p.h > 0.0 { ((r.y - p.y) / p.h) * 100.0 } else { 0.0 };
                                    let inner_w = if p.w > 0.0 { (r.w / p.w) * 100.0 } else { 100.0 };
                                    let inner_h = if p.h > 0.0 { (r.h / p.h) * 100.0 } else { 100.0 };
                                    let ghost_style = format!(
                                        "left: {:.2}%; top: {:.2}%; width: {:.2}%; height: {:.2}%;",
                                        inner_left, inner_top, inner_w, inner_h
                                    );

                                    let conf_pct = (interp.confidence * 100.0).round() as u32;
                                    let track_id = track.track_id;
                                    let is_face = track.kind == RedactionKind::Face;
                                    let is_wl = track.whitelisted;
                                    let is_enabled = track.enabled;

                                    rsx! {
                                        div {
                                            key: "rbox-{track_id}",
                                            class: "{box_class}",
                                            style: "{box_style}",
                                            onclick: move |evt| {
                                                evt.stop_propagation();
                                                state.selected_track_id.set(Some(track_id));
                                                state.mode.set(StudioMode::Shield);
                                            },
                                            ondoubleclick: move |evt| {
                                                evt.stop_propagation();
                                                if is_face {
                                                    state.push_undo();
                                                    let mut becoming_wl = false;
                                                    if let Some(t) = state.tracks.write().get_mut(idx) {
                                                        t.whitelisted = !t.whitelisted;
                                                        t.enabled = !t.whitelisted;
                                                        becoming_wl = t.whitelisted;
                                                    }
                                                    if becoming_wl {
                                                        state.track_verdicts.write().insert(track_id, TrackVerdict::Whitelisted);
                                                        state.notify(ToastSeverity::Success, format!("Face #{track_id} saved to Creator Whitelist"));
                                                        spawn(async move {
                                                            if is_tauri_runtime() {
                                                                let _ = cmd_whitelist_creator_face(
                                                                    format!("creator-{track_id}"),
                                                                    "Host".into(),
                                                                    vec![0.6, 0.8],
                                                                ).await;
                                                            }
                                                        });
                                                    } else {
                                                        state.track_verdicts.write().remove(&track_id);
                                                        state.notify(ToastSeverity::Info, format!("Removed Face #{track_id} from Creator Whitelist"));
                                                    }
                                                }
                                            },

                                            if show_masks && is_enabled && !is_wl && !is_rejected {
                                                div { class: "{mask_class}" }
                                            }

                                            if show_margin && cfg.spatial_padding_ratio > 0.001 && !is_rejected {
                                                div { class: "tc-rbox-ghost", style: "{ghost_style}" }
                                            }

                                            span {
                                                class: "tc-rbox__tag {tag_kind} tc-mono",
                                                "{kind_label} #{track_id}"
                                            }

                                            if is_selected {
                                                span { class: "tc-rbox__handle tc-rbox__handle--tl" }
                                                span { class: "tc-rbox__handle tc-rbox__handle--tr" }
                                                span { class: "tc-rbox__handle tc-rbox__handle--bl" }
                                                span { class: "tc-rbox__handle tc-rbox__handle--br" }
                                            }

                                            div {
                                                class: "tc-rbox__pill",
                                                onclick: move |evt| evt.stop_propagation(),
                                                span { class: "tc-rbox__pill-conf tc-mono", "{conf_pct}%" }
                                                if is_face {
                                                    button {
                                                        class: "tc-rbox__pill-btn tc-rbox__pill-btn--wl",
                                                        onclick: move |_| {
                                                            state.push_undo();
                                                            let mut becoming_wl = false;
                                                            if let Some(t) = state.tracks.write().get_mut(idx) {
                                                                t.whitelisted = !t.whitelisted;
                                                                t.enabled = !t.whitelisted;
                                                                becoming_wl = t.whitelisted;
                                                            }
                                                            if becoming_wl {
                                                                state.track_verdicts.write().insert(track_id, TrackVerdict::Whitelisted);
                                                                state.notify(ToastSeverity::Success, "Creator face whitelisted locally");
                                                                spawn(async move {
                                                                    if is_tauri_runtime() {
                                                                        let _ = cmd_whitelist_creator_face(
                                                                            format!("creator-{track_id}"),
                                                                            "Host".into(),
                                                                            vec![0.6, 0.8],
                                                                        ).await;
                                                                    }
                                                                });
                                                            } else {
                                                                state.track_verdicts.write().remove(&track_id);
                                                            }
                                                        },
                                                        if is_wl { "Un-whitelist" } else { "Keep Creator" }
                                                    }
                                                }
                                                button {
                                                    class: "tc-rbox__pill-btn tc-rbox__pill-btn--rej",
                                                    onclick: move |_| {
                                                        state.push_undo();
                                                        if let Some(t) = state.tracks.write().get_mut(idx) {
                                                            t.enabled = false;
                                                            t.whitelisted = false;
                                                        }
                                                        state.track_verdicts.write().insert(track_id, TrackVerdict::NotPii);
                                                        state.notify(ToastSeverity::Info, format!("Track #{track_id} marked Not PII"));
                                                    },
                                                    "Not PII"
                                                }
                                                button {
                                                    class: "tc-rbox__pill-btn",
                                                    onclick: move |_| {
                                                        state.push_undo();
                                                        if let Some(t) = state.tracks.write().get_mut(idx) {
                                                            t.enabled = !t.enabled;
                                                            if t.enabled {
                                                                t.whitelisted = false;
                                                                state.track_verdicts.write().remove(&track_id);
                                                            }
                                                        }
                                                    },
                                                    if is_enabled { "Disable" } else { "Blur" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Accurate Non-Glass Burn-In Subtitle Preview
                    if show_subs
                        && let Some(seg) = active_seg
                    {
                        div { class: "tc-sub-overlay",
                            if let Some(speaker) = seg.speaker.as_deref() {
                                span { class: "tc-sub-overlay__speaker", "{speaker}" }
                            }
                            div { class: "tc-sub-overlay__box",
                                if is_active_beeped {
                                    span { class: "tc-sub-overlay__beep", "[BEEP]" }
                                }
                                "{seg.text}"
                            }
                        }
                    }
                }

                // Floating Liquid Glass Transport Capsule in Viewer Apron (Never covers video/subtitles)
                div { class: "tc-transport",
                    div { class: "tc-transport__group",
                        button {
                            class: "tc-transport__icon",
                            title: "Jump to start",
                            onclick: move |_| state.seek_ms(0),
                            IconSkipBack {}
                        }
                        button {
                            class: "tc-transport__icon",
                            title: "Step back 1 frame",
                            onclick: move |_| {
                                let step = (1000.0 / fps).round() as u64;
                                let cur = *state.current_ms.read();
                                state.seek_ms(cur.saturating_sub(step.max(16)));
                            },
                            IconStepBack {}
                        }
                        button {
                            class: "tc-transport__play",
                            title: "Play / Pause (Space)",
                            onclick: move |_| state.toggle_play(),
                            if playing {
                                IconPause {}
                            } else {
                                IconPlay {}
                            }
                        }
                        button {
                            class: "tc-transport__icon",
                            title: "Step forward 1 frame",
                            onclick: move |_| {
                                let step = (1000.0 / fps).round() as u64;
                                let cur = *state.current_ms.read();
                                state.seek_ms(cur + step.max(16));
                            },
                            IconStepForward {}
                        }
                    }

                    div { class: "tc-transport__tc tc-mono",
                        span { "{format_smpte(current_ms, fps)}" }
                        span { class: "tc-transport__tc-total", " / {format_smpte(project.media.duration_ms, fps)}" }
                    }

                    div { class: "tc-transport__group tc-transport__group--end",
                        button {
                            class: "tc-transport__rate tc-mono",
                            title: "Cycle playback speed (J / K / L)",
                            onclick: move |_| {
                                let next = if (rate - 1.0).abs() < 0.05 {
                                    1.5
                                } else if (rate - 1.5).abs() < 0.05 {
                                    2.0
                                } else {
                                    1.0
                                };
                                state.playback_rate.set(next);
                            },
                            "{rate:.1}×"
                        }
                    }
                }
            }
        }
    }
}
