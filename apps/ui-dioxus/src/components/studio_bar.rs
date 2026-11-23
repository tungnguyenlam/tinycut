use crate::bridge::{cmd_save_project, is_tauri_runtime};
use crate::components::icons::{
    IconBrand, IconCheck, IconChevronDown, IconExport, IconLock, IconMoon, IconPlay, IconPlus,
    IconRedo, IconSun, IconUndo,
};
use crate::domain_ext::format_seg_time;
use crate::state::{StudioMode, StudioState, StudioTheme, ToastSeverity};
use dioxus::prelude::*;
use tinycut_core_domain::InferenceMode;

#[component]
pub fn StudioBar() -> Element {
    let mut state = use_context::<StudioState>();
    let project = state.project.read().clone();
    let mode = *state.mode.read();
    let theme = *state.theme.read();
    let dirty = *state.dirty.read();
    let show_proj = *state.show_project_menu.read();
    let show_priv = *state.show_privacy_menu.read();
    let recent = state.recent_projects.read().clone();
    let can_undo = !state.undo_stack.read().is_empty();
    let can_redo = !state.redo_stack.read().is_empty();

    let res_label = if project.media.width >= 3840 {
        "4K"
    } else if project.media.width >= 1920 {
        "1080p"
    } else if project.media.width > 0 {
        "HD"
    } else {
        "Audio"
    };

    let mode_title = match mode {
        StudioMode::Media => "Media & Projects",
        StudioMode::Subtitles => "Subtitles & 3-Way Diff",
        StudioMode::Shield => "Visual Privacy Shield",
        StudioMode::Glossary => "Personal Glossary",
    };

    fn persist_project(mut state: StudioState, updated: tinycut_core_domain::Project) {
        state.project.set(updated.clone());
        state.dirty.set(false);
        spawn(async move {
            if is_tauri_runtime() {
                let _ = cmd_save_project(updated).await;
            }
        });
    }

    rsx! {
        header { class: "tc-bar", "data-tauri-drag-region": "true",
            // Left Floating Liquid Glass Capsule: Brand + Project Menu
            div { class: "tc-capsule tc-glass",
                div { class: "tc-brand",
                    div { class: "tc-brand__mark", IconBrand {} }
                    span { "TinyCut" }
                }
                div { class: "tc-sep-v" }
                button {
                    class: "tc-proj-trigger",
                    title: "Switch project or import media",
                    onclick: move |_| {
                        let next = !*state.show_project_menu.read();
                        state.close_overlays();
                        state.show_project_menu.set(next);
                    },
                    span {
                        class: if dirty { "tc-saved-dot tc-saved-dot--dirty" } else { "tc-saved-dot" },
                        title: if dirty { "Unsaved edits" } else { "Saved locally" }
                    }
                    span { class: "tc-proj-trigger__name", "{project.title}" }
                    span { class: "tc-proj-trigger__meta tc-mono",
                        "{res_label} · {project.media.fps:.0}fps"
                    }
                    span { class: "tc-muted", IconChevronDown {} }
                }
            }

            // Center: Quiet Current Mode Title (no duplicated mode switcher)
            div { class: "tc-bar__center",
                span { "{mode_title}" }
            }

            // Right Floating Liquid Glass Capsule: Consolidated Privacy + Undo/Redo + Theme + Export
            div { class: "tc-capsule tc-glass",
                button {
                    class: "tc-privacy-btn",
                    title: "Inference & Privacy Posture",
                    onclick: move |_| {
                        let next = !*state.show_privacy_menu.read();
                        state.close_overlays();
                        state.show_privacy_menu.set(next);
                    },
                    span {
                        class: if project.inference_mode == InferenceMode::CloudBoost && !project.is_nda_private {
                            "tc-privacy-btn__dot tc-privacy-btn__dot--cloud"
                        } else {
                            "tc-privacy-btn__dot"
                        }
                    }
                    span {
                        if project.inference_mode == InferenceMode::CloudBoost && !project.is_nda_private {
                            "Cloud Boost"
                        } else {
                            "Local"
                        }
                    }
                    if project.is_nda_private {
                        span { class: "tc-privacy-btn__nda",
                            "·"
                            IconLock {}
                            "NDA"
                        }
                    }
                    span { class: "tc-muted", IconChevronDown {} }
                }

                div { class: "tc-sep-v" }

                button {
                    class: "tc-btn tc-btn--ghost tc-btn--icon",
                    disabled: !can_undo,
                    title: "Undo (Ctrl/⌘+Z)",
                    onclick: move |_| state.undo(),
                    IconUndo {}
                }
                button {
                    class: "tc-btn tc-btn--ghost tc-btn--icon",
                    disabled: !can_redo,
                    title: "Redo (Ctrl/⌘+Shift+Z)",
                    onclick: move |_| state.redo(),
                    IconRedo {}
                }

                button {
                    class: "tc-btn tc-btn--ghost tc-btn--icon",
                    title: if theme == StudioTheme::Light {
                        "Switch to Dark Theme (Ctrl/⌘+Shift+L)"
                    } else {
                        "Switch to Light Theme (Ctrl/⌘+Shift+L)"
                    },
                    onclick: move |_| state.toggle_theme(),
                    if theme == StudioTheme::Light {
                        IconMoon {}
                    } else {
                        IconSun {}
                    }
                }

                div { class: "tc-sep-v" }

                button {
                    class: "tc-btn tc-btn--primary",
                    title: "Export Subtitles & Timeline (Ctrl/⌘+E)",
                    onclick: move |_| {
                        state.close_overlays();
                        state.show_export_drawer.set(true);
                    },
                    IconExport {}
                    "Export"
                }
            }
        }

        // Project Switcher Glass Popover
        if show_proj {
            div {
                class: "tc-click-away",
                onclick: move |_| state.show_project_menu.set(false),
            }
            div { class: "tc-popover tc-popover--project tc-glass-thick",
                div { class: "tc-popover__heading", "Recent Projects" }
                for proj in recent.iter().cloned() {
                    {
                        let is_cur = proj.id == project.id;
                        let dur = format_seg_time(proj.media.duration_ms);
                        rsx! {
                            button {
                                key: "{proj.id}",
                                class: if is_cur { "tc-popover__item tc-popover__item--active" } else { "tc-popover__item" },
                                onclick: move |_| {
                                    state.project.set(proj.clone());
                                    state.show_project_menu.set(false);
                                    state.notify(ToastSeverity::Info, format!("Opened {}", proj.title));
                                },
                                div {
                                    div { "{proj.title}" }
                                    div { class: "tc-mono tc-muted",
                                        "{proj.media.width}×{proj.media.height} · {dur}"
                                    }
                                }
                                if proj.is_nda_private {
                                    span { class: "tc-privacy-btn__nda", IconLock {} }
                                }
                            }
                        }
                    }
                }
                div { class: "tc-popover__sep" }
                button {
                    class: "tc-popover__item",
                    onclick: move |_| {
                        state.mode.set(StudioMode::Media);
                        state.show_project_menu.set(false);
                    },
                    span { class: "tc-flex-row",
                        IconPlus {}
                        "Import local media…"
                    }
                    span { class: "tc-kbd", "⌘O" }
                }
                button {
                    class: "tc-popover__item",
                    onclick: move |_| {
                        state.load_demo();
                        state.show_project_menu.set(false);
                    },
                    span { class: "tc-flex-row",
                        IconPlay {}
                        "Reload 4K Street Vlog Demo"
                    }
                }
            }
        }

        // Consolidated Privacy & Inference Glass Popover
        if show_priv {
            div {
                class: "tc-click-away",
                onclick: move |_| state.show_privacy_menu.set(false),
            }
            div { class: "tc-popover tc-popover--privacy tc-glass-thick",
                div { class: "tc-popover__heading", "Inference Mode" }

                button {
                    class: if project.inference_mode == InferenceMode::Local {
                        "tc-popover__item tc-popover__item--active"
                    } else {
                        "tc-popover__item"
                    },
                    onclick: move |_| {
                        let mut p = state.project.read().clone();
                        p.inference_mode = InferenceMode::Local;
                        persist_project(state, p);
                        state.notify(ToastSeverity::Success, "Switched to Local (100% offline)");
                    },
                    div {
                        div { "Local On-Device" }
                        div { class: "tc-form-row__sub", "Whisper + Qwen + ONNX. Zero internet required." }
                    }
                    if project.inference_mode == InferenceMode::Local {
                        IconCheck {}
                    }
                }

                button {
                    class: if project.inference_mode == InferenceMode::CloudBoost && !project.is_nda_private {
                        "tc-popover__item tc-popover__item--active"
                    } else {
                        "tc-popover__item"
                    },
                    disabled: project.is_nda_private,
                    onclick: move |_| {
                        if state.project.read().is_nda_private {
                            return;
                        }
                        let mut p = state.project.read().clone();
                        p.inference_mode = InferenceMode::CloudBoost;
                        persist_project(state, p);
                        state.notify(ToastSeverity::Info, "Cloud Boost ready (proxy-only offload)");
                    },
                    div {
                        div { "Cloud Boost (Pro)" }
                        div { class: "tc-form-row__sub",
                            if project.is_nda_private {
                                "Blocked while NDA Privacy Lock is enabled."
                            } else {
                                "Uploads compact Opus/720p proxy only; raw 4K stays local."
                            }
                        }
                    }
                    if project.inference_mode == InferenceMode::CloudBoost && !project.is_nda_private {
                        IconCheck {}
                    }
                }

                div { class: "tc-popover__sep" }

                div { class: "tc-form-row",
                    div {
                        div { class: "tc-form-row__label",
                            IconLock {}
                            "NDA Privacy Lock"
                        }
                        div { class: "tc-form-row__sub", "Blocks Cloud Boost & all telemetry" }
                    }
                    button {
                        class: if project.is_nda_private {
                            "tc-switch tc-switch--warn tc-switch--on"
                        } else {
                            "tc-switch tc-switch--warn"
                        },
                        role: "switch",
                        "aria-checked": "{project.is_nda_private}",
                        onclick: move |_| {
                            let mut p = state.project.read().clone();
                            p.is_nda_private = !p.is_nda_private;
                            if p.is_nda_private {
                                p.inference_mode = InferenceMode::Local;
                                state.opt_in_telemetry.set(false);
                                state.notify(
                                    ToastSeverity::Warning,
                                    "NDA Lock ON — Cloud Boost & telemetry blocked",
                                );
                            } else {
                                state.notify(ToastSeverity::Info, "NDA Lock disabled");
                            }
                            persist_project(state, p);
                        },
                    }
                }
            }
        }
    }
}
