mod bridge;
mod components;
mod domain_ext;
mod state;

use bridge::{cmd_list_glossary, cmd_list_models, cmd_list_recent_projects, is_tauri_runtime};
use components::export_drawer::ExportDrawer;
use components::inspector_glossary::GlossaryInspector;
use components::inspector_media::MediaInspector;
use components::inspector_shield::ShieldInspector;
use components::inspector_subtitles::SubtitlesInspector;
use components::nav_rail::NavRail;
use components::overlays::{ShortcutsModal, StatusBar, ToastStack};
use components::stage::Stage;
use components::studio_bar::StudioBar;
use components::timeline::Timeline;
use dioxus::prelude::*;
use state::{StudioMode, StudioState, ToastSeverity, TrackVerdict};
use std::cell::RefCell;
use std::rc::Rc;
use tinycut_core_domain::RedactionKind;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

const STUDIO_CSS: &str = include_str!("../assets/studio.css");
type RafCallback = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut state = use_context_provider(StudioState::new);

    // Hydrate persisted SQLite glossary & recent projects on startup when running inside Tauri
    use_effect(move || {
        spawn(async move {
            if is_tauri_runtime() {
                if let Ok(entries) = cmd_list_glossary().await
                    && !entries.is_empty()
                {
                    state.glossary.set(entries);
                }
                if let Ok(projects) = cmd_list_recent_projects().await
                    && !projects.is_empty()
                {
                    state.recent_projects.set(projects);
                }
                if let Ok(models) = cmd_list_models().await
                    && !models.is_empty()
                {
                    state.models.set(models);
                }
            }
        });
    });

    // Smooth 60fps requestAnimationFrame playback ticker
    use_effect(move || {
        let win = match web_sys::window() {
            Some(w) => w,
            None => return,
        };
        let perf = win.performance();
        let last_ts = Rc::new(RefCell::new(
            perf.as_ref().map(|p| p.now()).unwrap_or(0.0),
        ));
        let f: RafCallback = Rc::new(RefCell::new(None));
        let g = f.clone();
        let win_clone = win.clone();

        *g.borrow_mut() = Some(Closure::new(move |ts: f64| {
            let prev = *last_ts.borrow();
            *last_ts.borrow_mut() = ts;
            let dt_ms = (ts - prev).clamp(0.0, 100.0);

            if *state.playing.read() {
                let rate = *state.playback_rate.read() as f64;
                let dur = state.project.read().media.duration_ms.max(1);
                let cur = *state.current_ms.read();
                let next = (cur as f64 + dt_ms * rate).round() as u64;
                if next >= dur {
                    state.current_ms.set(dur);
                    state.playing.set(false);
                } else {
                    state.current_ms.set(next);
                }
            }

            if let Some(cb) = f.borrow().as_ref() {
                let _ = win_clone.request_animation_frame(cb.as_ref().unchecked_ref());
            }
        }));

        if let Some(cb) = g.borrow().as_ref() {
            let _ = win.request_animation_frame(cb.as_ref().unchecked_ref());
        }
    });

    // Global NLE Keyboard Shortcuts
    let on_keydown = move |evt: KeyboardEvent| {
        let key = evt.key();
        let mods = evt.modifiers();
        let cmd_or_ctrl = mods.ctrl() || mods.meta();
        let editing = *state.editing_text.read();

        if key == Key::Escape {
            state.close_overlays();
            return;
        }

        if cmd_or_ctrl {
            match &key {
                Key::Character(c) if c.eq_ignore_ascii_case("e") => {
                    evt.prevent_default();
                    let cur = *state.show_export_drawer.read();
                    state.close_overlays();
                    state.show_export_drawer.set(!cur);
                    return;
                }
                Key::Character(c) if c.eq_ignore_ascii_case("o") => {
                    evt.prevent_default();
                    state.mode.set(StudioMode::Media);
                    return;
                }
                Key::Character(c) if c.eq_ignore_ascii_case("l") && mods.shift() => {
                    evt.prevent_default();
                    state.toggle_theme();
                    return;
                }
                Key::Character(c) if c == "=" || c == "+" => {
                    evt.prevent_default();
                    let cur = *state.timeline_zoom.read();
                    state.timeline_zoom.set((cur + 20.0).min(240.0));
                    return;
                }
                Key::Character(c) if c == "-" => {
                    evt.prevent_default();
                    let cur = *state.timeline_zoom.read();
                    state.timeline_zoom.set((cur - 20.0).max(32.0));
                    return;
                }
                Key::Character(c) if c.eq_ignore_ascii_case("z") && !editing => {
                    evt.prevent_default();
                    if mods.shift() {
                        state.redo();
                    } else {
                        state.undo();
                    }
                    return;
                }
                _ => {}
            }
        }

        if editing {
            return;
        }

        match key {
            Key::Character(ref c) if c == " " => {
                evt.prevent_default();
                state.toggle_play();
            }
            Key::Character(ref c) if c.eq_ignore_ascii_case("k") => {
                state.playing.set(false);
                state.playback_rate.set(1.0);
            }
            Key::Character(ref c) if c.eq_ignore_ascii_case("l") => {
                let r = *state.playback_rate.read();
                let playing = *state.playing.read();
                let next_r = if playing { (r * 1.5).min(2.0) } else { 1.0 };
                state.playback_rate.set(next_r);
                state.playing.set(true);
            }
            Key::Character(ref c) if c.eq_ignore_ascii_case("j") => {
                let cur = *state.current_ms.read();
                state.playing.set(false);
                state.seek_ms(cur.saturating_sub(1_000));
            }
            Key::Character(ref c) if c.eq_ignore_ascii_case("d") => {
                let cur = *state.show_diffs.read();
                state.show_diffs.set(!cur);
            }
            Key::Character(ref c) if c.eq_ignore_ascii_case("m") => {
                let cur = *state.show_masks.read();
                state.show_masks.set(!cur);
            }
            Key::Character(ref c) if c == "?" => {
                let cur = *state.show_shortcuts_modal.read();
                state.close_overlays();
                state.show_shortcuts_modal.set(!cur);
            }
            Key::Character(ref c) if c.eq_ignore_ascii_case("w") => {
                let selected = *state.selected_track_id.read();
                if let Some(tid) = selected {
                    let mut changed = false;
                    let mut becoming_wl = false;
                    state.push_undo();
                    for t in state.tracks.write().iter_mut() {
                        if t.track_id == tid && t.kind == RedactionKind::Face {
                            t.whitelisted = !t.whitelisted;
                            t.enabled = !t.whitelisted;
                            becoming_wl = t.whitelisted;
                            changed = true;
                        }
                    }
                    if changed {
                        if becoming_wl {
                            state.track_verdicts.write().insert(tid, TrackVerdict::Whitelisted);
                            state.notify(ToastSeverity::Success, format!("Whitelisted Creator Face #{tid}"));
                        } else {
                            state.track_verdicts.write().remove(&tid);
                            state.notify(ToastSeverity::Info, format!("Un-whitelisted Face #{tid}"));
                        }
                    }
                }
            }
            Key::Character(ref c) if c.eq_ignore_ascii_case("n") => {
                let selected = *state.selected_track_id.read();
                if let Some(tid) = selected {
                    state.push_undo();
                    for t in state.tracks.write().iter_mut() {
                        if t.track_id == tid {
                            t.enabled = false;
                            t.whitelisted = false;
                        }
                    }
                    state.track_verdicts.write().insert(tid, TrackVerdict::NotPii);
                    state.notify(ToastSeverity::Info, format!("Marked Track #{tid} as Not PII"));
                }
            }
            Key::ArrowUp | Key::ArrowDown => {
                let segs = state.transcript.read().segments.clone();
                if segs.is_empty() {
                    return;
                }
                let cur_id = *state.selected_segment_id.read();
                let cur_idx = cur_id
                    .and_then(|id| segs.iter().position(|s| s.id == id))
                    .unwrap_or(0);
                let next_idx = if key == Key::ArrowDown {
                    (cur_idx + 1).min(segs.len() - 1)
                } else {
                    cur_idx.saturating_sub(1)
                };
                if let Some(seg) = segs.get(next_idx) {
                    state.selected_segment_id.set(Some(seg.id));
                    state.seek_ms(seg.start_ms);
                }
            }
            _ => {}
        }
    };

    let mode = *state.mode.read();

    rsx! {
        style { "{STUDIO_CSS}" }

        div {
            class: "tc-studio",
            tabindex: "0",
            onkeydown: on_keydown,

            // Top Toolbar Band with Floating Liquid Glass Capsules
            StudioBar {}

            // Left Inset Tahoe Liquid Glass Icon Rail (Only Mode Switcher)
            NavRail {}

            // Central Neutral Viewer Surround + Floating Glass Transport Capsule
            Stage {}

            // Right Opaque Mode-Aware Inspector
            aside { class: "tc-inspector",
                match mode {
                    StudioMode::Subtitles => rsx! { SubtitlesInspector {} },
                    StudioMode::Shield => rsx! { ShieldInspector {} },
                    StudioMode::Glossary => rsx! { GlossaryInspector {} },
                    StudioMode::Media => rsx! { MediaInspector {} },
                }
            }

            // Bottom Zoomable Opaque Multi-Track Timeline
            Timeline {}

            // Bottom Slim Flat Status Bar (22px)
            StatusBar {}

            // Floating Glass Overlays
            ExportDrawer {}
            ShortcutsModal {}
            ToastStack {}
        }
    }
}
