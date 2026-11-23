use crate::components::icons::{
    IconGlossary, IconKeyboard, IconMedia, IconShield, IconSubtitles,
};
use crate::state::{StudioMode, StudioState};
use dioxus::prelude::*;

#[component]
pub fn NavRail() -> Element {
    let mut state = use_context::<StudioState>();
    let mode = *state.mode.read();
    let seg_count = state.transcript.read().segments.len();
    let active_tracks = state
        .tracks
        .read()
        .iter()
        .filter(|t| t.enabled && !t.whitelisted)
        .count();
    let gloss_count = state.glossary.read().len();

    rsx! {
        div { class: "tc-rail-slot",
            nav { class: "tc-rail tc-glass", "aria-label": "Studio Navigation",
                div { class: "tc-rail__group",
                    button {
                        class: if mode == StudioMode::Media { "tc-rail__btn tc-rail__btn--active" } else { "tc-rail__btn" },
                        title: "Media & Projects (Ctrl/⌘+O)",
                        "aria-label": "Media & Projects",
                        onclick: move |_| state.mode.set(StudioMode::Media),
                        IconMedia {}
                    }

                    button {
                        class: if mode == StudioMode::Subtitles { "tc-rail__btn tc-rail__btn--active" } else { "tc-rail__btn" },
                        title: "Subtitles & 3-Way Diff ({seg_count})",
                        "aria-label": "Subtitles & 3-Way Diff",
                        onclick: move |_| state.mode.set(StudioMode::Subtitles),
                        IconSubtitles {}
                        if seg_count > 0 {
                            span { class: "tc-rail__badge tc-mono", "{seg_count}" }
                        }
                    }

                    button {
                        class: if mode == StudioMode::Shield { "tc-rail__btn tc-rail__btn--active" } else { "tc-rail__btn" },
                        title: "Visual Privacy Shield ({active_tracks} active)",
                        "aria-label": "Visual Privacy Shield",
                        onclick: move |_| state.mode.set(StudioMode::Shield),
                        IconShield {}
                        if active_tracks > 0 {
                            span { class: "tc-rail__badge tc-rail__badge--alert tc-mono", "{active_tracks}" }
                        }
                    }

                    button {
                        class: if mode == StudioMode::Glossary { "tc-rail__btn tc-rail__btn--active" } else { "tc-rail__btn" },
                        title: "Personal Glossary ({gloss_count})",
                        "aria-label": "Personal Glossary",
                        onclick: move |_| state.mode.set(StudioMode::Glossary),
                        IconGlossary {}
                        if gloss_count > 0 {
                            span { class: "tc-rail__badge tc-mono", "{gloss_count}" }
                        }
                    }
                }

                div { class: "tc-rail__group",
                    button {
                        class: "tc-rail__btn",
                        title: "Keyboard Shortcuts (?)",
                        "aria-label": "Keyboard Shortcuts",
                        onclick: move |_| {
                            let cur = *state.show_shortcuts_modal.read();
                            state.close_overlays();
                            state.show_shortcuts_modal.set(!cur);
                        },
                        IconKeyboard {}
                    }
                }
            }
        }
    }
}
