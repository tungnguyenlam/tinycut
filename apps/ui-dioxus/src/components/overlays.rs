use crate::components::icons::{IconAlert, IconCheck, IconClose};
use crate::domain_ext::ms_to_frame;
use crate::state::{StudioState, ToastSeverity};
use dioxus::prelude::*;
use tinycut_core_domain::InferenceMode;

#[component]
pub fn StatusBar() -> Element {
    let state = use_context::<StudioState>();
    let project = state.project.read().clone();
    let current_ms = *state.current_ms.read();
    let fps = project.media.fps.max(1.0);
    let cur_frame = ms_to_frame(current_ms, fps);
    let total_frames = ms_to_frame(project.media.duration_ms.max(1), fps);
    let active_masks = state
        .tracks
        .read()
        .iter()
        .filter(|t| t.enabled && !t.whitelisted)
        .count();
    let (sev, msg) = state.status_msg.read().clone();
    let busy = state.busy_job.read().clone();

    let msg_cls = match sev {
        ToastSeverity::Info => "tc-status__msg--info",
        ToastSeverity::Success => "tc-status__msg--success",
        ToastSeverity::Warning => "tc-status__msg--warning",
        ToastSeverity::Error => "tc-status__msg--error",
    };

    rsx! {
        footer { class: "tc-status",
            div { class: "tc-status__group",
                if let Some(job) = busy {
                    span { "{job}…" }
                } else {
                    span { class: "{msg_cls}", "{msg}" }
                }
            }

            div { class: "tc-status__group tc-mono",
                span { "Frame {cur_frame} / {total_frames}" }
                span { class: "tc-status__sep", "·" }
                span { "{active_masks} shield" }
                span { class: "tc-status__sep", "·" }
                span {
                    if project.inference_mode == InferenceMode::Local {
                        "Local"
                    } else {
                        "Cloud Boost"
                    }
                }
            }
        }
    }
}

#[component]
pub fn ToastStack() -> Element {
    let mut state = use_context::<StudioState>();
    let toasts = state.toasts.read().clone();
    if toasts.is_empty() {
        return rsx! {};
    }

    rsx! {
        div { class: "tc-toasts",
            for t in toasts {
                {
                    let icon_cls = match t.severity {
                        ToastSeverity::Info => "tc-toast__icon tc-toast__icon--info",
                        ToastSeverity::Success => "tc-toast__icon tc-toast__icon--success",
                        ToastSeverity::Warning => "tc-toast__icon tc-toast__icon--warning",
                        ToastSeverity::Error => "tc-toast__icon tc-toast__icon--error",
                    };
                    let id = t.id;
                    rsx! {
                        div { key: "toast-{id}", class: "tc-toast tc-glass-thick",
                            span { class: "{icon_cls}",
                                if t.severity == ToastSeverity::Success {
                                    IconCheck {}
                                } else {
                                    IconAlert {}
                                }
                            }
                            span { "{t.message}" }
                            button {
                                class: "tc-btn tc-btn--ghost tc-btn--sm tc-btn--icon",
                                onclick: move |_| state.dismiss_toast(id),
                                IconClose {}
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn ShortcutsModal() -> Element {
    let mut state = use_context::<StudioState>();
    if !*state.show_shortcuts_modal.read() {
        return rsx! {};
    }

    let shortcuts = [
        ("Play / Pause", "Space"),
        ("Reverse / Pause / Forward", "J / K / L"),
        ("Prev / Next Subtitle", "↑ / ↓"),
        ("Toggle Inline Diff", "D"),
        ("Toggle Blur Masks", "M"),
        ("Whitelist Selected Face", "W"),
        ("Mark Track Not PII", "N"),
        ("Timeline Zoom In / Out", "⌘+ / ⌘−"),
        ("Switch Light / Dark", "⌘⇧L"),
        ("Export", "⌘E"),
        ("Undo / Redo", "⌘Z / ⌘⇧Z"),
        ("Close Sheet / Popover", "Esc"),
    ];

    rsx! {
        div {
            class: "tc-backdrop",
            onclick: move |_| state.show_shortcuts_modal.set(false),
        }
        div { class: "tc-sheet tc-sheet--compact tc-glass-thick", role: "dialog", "aria-label": "Keyboard Shortcuts",
            div { class: "tc-sheet__head",
                div {
                    div { class: "tc-sheet__title", "Keyboard Shortcuts" }
                    div { class: "tc-sheet__sub", "Single-key shortcuts pause automatically while editing text." }
                }
                button {
                    class: "tc-btn tc-btn--ghost tc-btn--icon",
                    onclick: move |_| state.show_shortcuts_modal.set(false),
                    IconClose {}
                }
            }

            div { class: "tc-sheet__body",
                div { class: "tc-shortcut-grid",
                    for (desc, keys) in shortcuts {
                        div { key: "{desc}", class: "tc-shortcut-row",
                            span { "{desc}" }
                            span { class: "tc-kbd", "{keys}" }
                        }
                    }
                }
            }
        }
    }
}
