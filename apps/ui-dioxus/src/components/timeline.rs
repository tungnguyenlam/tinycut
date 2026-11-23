use crate::components::icons::{
    IconFace, IconMagnet, IconMonitor, IconPlate, IconSubtitles, IconWave, IconZoomIn, IconZoomOut,
};
use crate::domain_ext::{
    detect_spoken_pii, format_seg_time, frame_to_ms, ms_to_frame, track_frame_bounds,
};
use crate::state::{StudioMode, StudioState, ToastSeverity, TrackVerdict};
use dioxus::prelude::*;
use tinycut_core_domain::RedactionKind;

#[component]
pub fn Timeline() -> Element {
    let mut state = use_context::<StudioState>();

    let media = state.project.read().media.clone();
    let dur_ms = media.duration_ms.max(1);
    let fps = media.fps.max(1.0);
    let total_frames = ms_to_frame(dur_ms, fps).max(1);
    let current_ms = *state.current_ms.read();

    let px_per_sec = *state.timeline_zoom.read();
    let px_per_ms = px_per_sec / 1000.0;
    let snap_on = *state.timeline_snap.read();
    let content_width_px = ((dur_ms as f64) * px_per_ms).max(600.0);
    let playhead_px = ((current_ms as f64) * px_per_ms).clamp(0.0, content_width_px);

    let segments = state.transcript.read().segments.clone();
    let selected_seg = *state.selected_segment_id.read();
    let beeped = state.beeped_segments.read().clone();

    let tracks = state.tracks.read().clone();
    let selected_track = *state.selected_track_id.read();
    let verdicts = state.track_verdicts.read().clone();
    let cfg = state.shield_cfg.read().clone();

    // Silence spans between subtitle segments (Smart Auto-Cut)
    let mut silences: Vec<(u64, u64)> = Vec::new();
    let mut cursor = 0_u64;
    for seg in &segments {
        if seg.start_ms > cursor + 250 {
            silences.push((cursor, seg.start_ms));
        }
        cursor = cursor.max(seg.end_ms);
    }
    if dur_ms > cursor + 250 {
        silences.push((cursor, dur_ms));
    }

    // Zoom-adaptive ruler ticks (major + minor)
    let major_step_ms: u64 = if px_per_sec >= 140.0 {
        1_000
    } else if px_per_sec >= 65.0 {
        2_000
    } else {
        5_000
    };
    let minor_step_ms = major_step_ms / 2;
    let mut major_ticks = Vec::new();
    let mut minor_ticks = Vec::new();
    let mut t = 0_u64;
    while t <= dur_ms {
        major_ticks.push(t);
        if t + minor_step_ms < dur_ms {
            minor_ticks.push(t + minor_step_ms);
        }
        t += major_step_ms;
    }

    // Single mirrored SVG waveform path (replaces 96 DOM bars)
    let samples = 140_usize;
    let mut top_pts = Vec::with_capacity(samples + 2);
    let mut bot_pts = Vec::with_capacity(samples + 2);
    for i in 0..=samples {
        let ratio = i as f64 / samples as f64;
        let x = ratio * 1000.0;
        let t_ms = (ratio * dur_ms as f64).round() as u64;
        let in_speech = segments
            .iter()
            .any(|s| t_ms >= s.start_ms && t_ms <= s.end_ms);
        let amp = if in_speech {
            10.0 + (((i * 37 + 13) % 24) as f64)
        } else {
            2.0 + (((i * 19 + 5) % 4) as f64)
        };
        top_pts.push(format!("{x:.1},{:.1}", 50.0 - amp));
        bot_pts.push(format!("{x:.1},{:.1}", 50.0 + amp));
    }
    bot_pts.reverse();
    let wav_path = format!(
        "M {} L {} Z",
        top_pts.join(" L "),
        bot_pts.join(" L ")
    );

    let scrub_from_click = move |evt: MouseEvent| {
        if let Some(win) = web_sys::window()
            && let Some(doc) = win.document()
            && let Some(el) = doc.get_element_by_id("tc-timeline-canvas")
        {
            let rect = el.get_bounding_client_rect();
            if rect.width() > 0.0 {
                let x = (evt.client_coordinates().x - rect.left()).clamp(0.0, rect.width());
                let raw_ms = (x / px_per_ms).round() as u64;
                let target = if snap_on {
                    let frame_ms = (1000.0 / (fps as f64)).max(1.0);
                    ((raw_ms as f64 / frame_ms).round() * frame_ms) as u64
                } else {
                    raw_ms
                };
                state.seek_ms(target);
            }
        }
    };

    let plate_count = tracks.iter().filter(|t| t.kind == RedactionKind::LicensePlate).count();
    let face_count = tracks.iter().filter(|t| t.kind == RedactionKind::Face).count();
    let pii_count = tracks
        .iter()
        .filter(|t| matches!(t.kind, RedactionKind::ScreenPii | RedactionKind::QrCode))
        .count();

    let silence_count = silences.len();

    rsx! {
        section { class: "tc-timeline",
            // Slim NLE Timeline Toolbar (Zoom, Snap, Auto-Cut — zero help text bloat)
            div { class: "tc-tl-topbar",
                div { class: "tc-tl-topbar__group",
                    button {
                        class: "tc-btn tc-btn--ghost tc-btn--sm tc-btn--icon",
                        title: "Zoom out (Ctrl/⌘+-)",
                        onclick: move |_| {
                            let cur = *state.timeline_zoom.read();
                            state.timeline_zoom.set((cur - 20.0).max(32.0));
                        },
                        IconZoomOut {}
                    }
                    input {
                        class: "tc-zoom-slider",
                        r#type: "range",
                        min: "32",
                        max: "240",
                        step: "4",
                        value: "{px_per_sec:.0}",
                        title: "Timeline zoom ({px_per_sec:.0} px/s)",
                        oninput: move |e| {
                            if let Ok(v) = e.value().parse::<f64>() {
                                state.timeline_zoom.set(v.clamp(32.0, 240.0));
                            }
                        }
                    }
                    button {
                        class: "tc-btn tc-btn--ghost tc-btn--sm tc-btn--icon",
                        title: "Zoom in (Ctrl/⌘++)",
                        onclick: move |_| {
                            let cur = *state.timeline_zoom.read();
                            state.timeline_zoom.set((cur + 20.0).min(240.0));
                        },
                        IconZoomIn {}
                    }
                    span { class: "tc-sep-v" }
                    button {
                        class: if snap_on { "tc-btn tc-btn--sm tc-btn--active" } else { "tc-btn tc-btn--ghost tc-btn--sm" },
                        title: "Toggle frame-snapped scrubbing",
                        "aria-pressed": "{snap_on}",
                        onclick: move |_| state.timeline_snap.set(!snap_on),
                        IconMagnet {}
                        "Snap"
                    }
                }

                div { class: "tc-tl-topbar__group",
                    if silence_count > 0 {
                        button {
                            class: "tc-btn tc-btn--ghost tc-btn--sm",
                            title: "Preview Smart Auto-Cut silence spans",
                            onclick: move |_| {
                                state.notify(
                                    ToastSeverity::Info,
                                    format!("Detected {silence_count} silence span(s) ready for Smart Auto-Cut"),
                                );
                            },
                            "Auto-cut silences ({silence_count})"
                        }
                    }
                }
            }

            div { class: "tc-tl-body",
                // Fixed 156px Sentence-Case Track Headers with 3px Color Swatches
                div { class: "tc-tl-headers",
                    div { class: "tc-tl-header-cell tc-tl-header-cell--ruler tc-mono", "Timecode" }
                    div { class: "tc-tl-header-cell tc-tl-header-cell--sub",
                        span { class: "tc-tl-swatch tc-tl-swatch--sub" }
                        IconSubtitles {}
                        span { class: "tc-tl-header-name", "Subtitles" }
                        span { class: "tc-tl-header-count tc-mono", "{segments.len()}" }
                    }
                    div { class: "tc-tl-header-cell tc-tl-header-cell--wav",
                        span { class: "tc-tl-swatch tc-tl-swatch--wav" }
                        IconWave {}
                        span { class: "tc-tl-header-name", "Audio" }
                    }
                    div { class: "tc-tl-header-cell tc-tl-header-cell--red",
                        span { class: "tc-tl-swatch tc-tl-swatch--plate" }
                        IconPlate {}
                        span { class: "tc-tl-header-name", "Plates" }
                        span { class: "tc-tl-header-count tc-mono", "{plate_count}" }
                    }
                    div { class: "tc-tl-header-cell tc-tl-header-cell--red",
                        span { class: "tc-tl-swatch tc-tl-swatch--face" }
                        IconFace {}
                        span { class: "tc-tl-header-name", "Faces" }
                        span { class: "tc-tl-header-count tc-mono", "{face_count}" }
                    }
                    div { class: "tc-tl-header-cell tc-tl-header-cell--red",
                        span { class: "tc-tl-swatch tc-tl-swatch--pii" }
                        IconMonitor {}
                        span { class: "tc-tl-header-name", "Screen PII" }
                        span { class: "tc-tl-header-count tc-mono", "{pii_count}" }
                    }
                }

                // Horizontally Scrollable, Zoom-Scaled Multi-Track Canvas
                div { class: "tc-tl-scroll",
                    div {
                        id: "tc-timeline-canvas",
                        class: "tc-tl-canvas",
                        style: "width: {content_width_px:.0}px;",
                        onclick: scrub_from_click,

                        // 1. Zoom-Adaptive Ruler (24px)
                        div { class: "tc-tl-ruler",
                            for tick_ms in major_ticks {
                                {
                                    let left = (tick_ms as f64) * px_per_ms;
                                    rsx! {
                                        div {
                                            key: "maj-{tick_ms}",
                                            class: "tc-tl-tick tc-mono",
                                            style: "left: {left:.1}px;",
                                            "{format_seg_time(tick_ms)}"
                                        }
                                    }
                                }
                            }
                            for tick_ms in minor_ticks {
                                {
                                    let left = (tick_ms as f64) * px_per_ms;
                                    rsx! {
                                        div {
                                            key: "min-{tick_ms}",
                                            class: "tc-tl-tick tc-tl-tick--minor",
                                            style: "left: {left:.1}px;",
                                        }
                                    }
                                }
                            }
                        }

                        // 2. Subtitle Lane (40px)
                        div { class: "tc-tl-lane tc-tl-lane--sub",
                            for seg in segments.iter().cloned() {
                                {
                                    let left = (seg.start_ms as f64) * px_per_ms;
                                    let width = ((seg.end_ms.saturating_sub(seg.start_ms)) as f64 * px_per_ms).max(16.0);
                                    let is_active = selected_seg == Some(seg.id)
                                        || (current_ms >= seg.start_ms && current_ms <= seg.end_ms);
                                    rsx! {
                                        div {
                                            key: "tl-sub-{seg.id}",
                                            class: if is_active { "tc-tl-sub-clip tc-tl-sub-clip--active" } else { "tc-tl-sub-clip" },
                                            style: "left: {left:.1}px; width: {width:.1}px;",
                                            title: "{seg.text}",
                                            onclick: move |evt| {
                                                evt.stop_propagation();
                                                state.selected_segment_id.set(Some(seg.id));
                                                state.seek_ms(seg.start_ms);
                                                state.mode.set(StudioMode::Subtitles);
                                            },
                                            "{seg.text}"
                                        }
                                    }
                                }
                            }
                        }

                        // 3. Audio Waveform (Mirrored SVG Path) + Clean Silence Fill + PII Pins (48px)
                        div { class: "tc-tl-lane tc-tl-lane--alt tc-tl-lane--wav",
                            svg {
                                class: "tc-tl-wav-svg",
                                view_box: "0 0 1000 100",
                                preserve_aspect_ratio: "none",
                                path { d: "{wav_path}", fill: "currentColor" }
                            }
                            for (i, (s_start, s_end)) in silences.iter().enumerate() {
                                {
                                    let left = (*s_start as f64) * px_per_ms;
                                    let width = ((*s_end - *s_start) as f64) * px_per_ms;
                                    rsx! {
                                        div {
                                            key: "sil-{i}",
                                            class: "tc-tl-silence",
                                            style: "left: {left:.1}px; width: {width:.1}px;",
                                            title: "Silence span",
                                        }
                                    }
                                }
                            }
                            for seg in segments.iter() {
                                if detect_spoken_pii(&seg.text).is_some() {
                                    {
                                        let left = (seg.start_ms as f64) * px_per_ms;
                                        let width = ((seg.end_ms.saturating_sub(seg.start_ms)) as f64 * px_per_ms).max(8.0);
                                        let is_beep = beeped.contains(&seg.id);
                                        rsx! {
                                            div {
                                                key: "pii-wav-{seg.id}",
                                                class: "tc-tl-pii-pin",
                                                style: "left: {left:.1}px; width: {width:.1}px;",
                                                title: if is_beep { "Spoken PII Auto-Beep armed" } else { "Spoken PII detected" },
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // 4. License Plate Lane (30px)
                        {render_redaction_lane(&tracks, RedactionKind::LicensePlate, false, &cfg, &verdicts, selected_track, total_frames, fps, px_per_ms, state)}

                        // 5. Face Lane (30px)
                        {render_redaction_lane(&tracks, RedactionKind::Face, true, &cfg, &verdicts, selected_track, total_frames, fps, px_per_ms, state)}

                        // 6. Screen PII / QR Lane (30px)
                        {render_redaction_lane(&tracks, RedactionKind::ScreenPii, false, &cfg, &verdicts, selected_track, total_frames, fps, px_per_ms, state)}

                        // Crisp 1px Playhead Needle + Solid Cap
                        div {
                            class: "tc-playhead",
                            style: "left: {playhead_px:.1}px;",
                            div { class: "tc-playhead__head" }
                        }
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_redaction_lane(
    tracks: &[tinycut_core_domain::RedactionTrack],
    lane_kind: RedactionKind,
    alt_bg: bool,
    cfg: &tinycut_core_domain::ShieldConfig,
    verdicts: &std::collections::HashMap<u64, TrackVerdict>,
    selected_track: Option<u64>,
    total_frames: u64,
    fps: f32,
    px_per_ms: f64,
    mut state: StudioState,
) -> Element {
    let lane_tracks: Vec<_> = tracks
        .iter()
        .filter(|t| {
            if lane_kind == RedactionKind::ScreenPii {
                matches!(t.kind, RedactionKind::ScreenPii | RedactionKind::QrCode)
            } else {
                t.kind == lane_kind
            }
        })
        .cloned()
        .collect();

    let lane_cls = if alt_bg {
        "tc-tl-lane tc-tl-lane--alt tc-tl-lane--red"
    } else {
        "tc-tl-lane tc-tl-lane--red"
    };

    rsx! {
        div { class: "{lane_cls}",
            for track in lane_tracks {
                if let Some((f_start, f_end, p_start, p_end)) = track_frame_bounds(&track, cfg, total_frames) {
                    {
                        let s_ms = frame_to_ms(f_start, fps);
                        let e_ms = frame_to_ms(f_end, fps).max(s_ms + 150);
                        let ps_ms = frame_to_ms(p_start, fps);
                        let pe_ms = frame_to_ms(p_end, fps).max(e_ms);

                        let pad_left = (ps_ms as f64) * px_per_ms;
                        let pad_width = ((pe_ms.saturating_sub(ps_ms)) as f64 * px_per_ms).max(16.0);
                        let left = (s_ms as f64) * px_per_ms;
                        let width = ((e_ms.saturating_sub(s_ms)) as f64 * px_per_ms).max(20.0);

                        let is_rejected = verdicts.get(&track.track_id) == Some(&TrackVerdict::NotPii);
                        let is_selected = selected_track == Some(track.track_id);

                        let (base_cls, pad_cls) = if is_rejected {
                            ("tc-tl-rbar tc-tl-rbar--rejected", "")
                        } else if track.whitelisted {
                            ("tc-tl-rbar tc-tl-rbar--whitelist", "")
                        } else {
                            match track.kind {
                                RedactionKind::LicensePlate => ("tc-tl-rbar tc-tl-rbar--plate", "tc-tl-rbar-pad tc-tl-rbar-pad--plate"),
                                RedactionKind::Face => ("tc-tl-rbar tc-tl-rbar--face", "tc-tl-rbar-pad tc-tl-rbar-pad--face"),
                                RedactionKind::ScreenPii | RedactionKind::QrCode => ("tc-tl-rbar tc-tl-rbar--pii", "tc-tl-rbar-pad tc-tl-rbar-pad--pii"),
                            }
                        };

                        let bar_cls = if is_selected {
                            format!("{base_cls} tc-tl-rbar--selected")
                        } else {
                            base_cls.to_string()
                        };

                        let label = if track.whitelisted {
                            format!("Creator #{}", track.track_id)
                        } else if is_rejected {
                            format!("Not PII #{}", track.track_id)
                        } else {
                            format!("#{}", track.track_id)
                        };

                        rsx! {
                            if cfg.temporal_padding_frames > 0 && !pad_cls.is_empty() {
                                div {
                                    key: "pad-{track.track_id}",
                                    class: "{pad_cls}",
                                    style: "left: {pad_left:.1}px; width: {pad_width:.1}px;",
                                }
                            }
                            div {
                                key: "rbar-{track.track_id}",
                                class: "{bar_cls}",
                                style: "left: {left:.1}px; width: {width:.1}px;",
                                onclick: move |evt| {
                                    evt.stop_propagation();
                                    state.selected_track_id.set(Some(track.track_id));
                                    state.seek_ms(s_ms);
                                    state.mode.set(StudioMode::Shield);
                                },
                                "{label}"
                            }
                        }
                    }
                }
            }
        }
    }
}
