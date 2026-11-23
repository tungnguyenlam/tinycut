use crate::bridge::{cmd_delete_glossary_term, cmd_upsert_glossary_term, is_tauri_runtime};
use crate::components::icons::{IconClose, IconPlus, IconSearch};
use crate::state::{StudioState, ToastSeverity};
use dioxus::prelude::*;
use tinycut_core_domain::GlossaryEntry;

#[component]
pub fn GlossaryInspector() -> Element {
    let mut state = use_context::<StudioState>();
    let mut search = use_signal(String::new);
    let mut spoken_in = use_signal(String::new);
    let mut preferred_in = use_signal(String::new);

    let entries = state.glossary.read().clone();
    let q = search.read().trim().to_lowercase();
    let filtered: Vec<GlossaryEntry> = entries
        .iter()
        .filter(|e| {
            q.is_empty()
                || e.spoken_variant.to_lowercase().contains(&q)
                || e.preferred_term.to_lowercase().contains(&q)
        })
        .cloned()
        .collect();

    let prompt_terms: Vec<String> = entries.iter().map(|e| e.preferred_term.clone()).collect();
    let prompt_preview = if prompt_terms.is_empty() {
        "No glossary terms yet.".to_string()
    } else {
        format!("Glossary terms: {}.", prompt_terms.join(", "))
    };

    let on_add = move |_| {
        let s = spoken_in.read().trim().to_string();
        let p = preferred_in.read().trim().to_string();
        if s.is_empty() || p.is_empty() {
            state.notify(
                ToastSeverity::Warning,
                "Enter both a spoken variant and preferred term",
            );
            return;
        }
        spoken_in.set(String::new());
        preferred_in.set(String::new());

        spawn(async move {
            if is_tauri_runtime() {
                match cmd_upsert_glossary_term(s.clone(), p.clone()).await {
                    Ok(list) => {
                        state.glossary.set(list);
                        state.notify(ToastSeverity::Success, format!("Saved to Glossary: {s} → {p}"));
                    }
                    Err(e) => state.notify(ToastSeverity::Error, format!("Glossary save failed: {e}")),
                }
            } else {
                {
                    let mut list = state.glossary.write();
                    if let Some(item) = list.iter_mut().find(|g| g.spoken_variant.eq_ignore_ascii_case(&s)) {
                        item.preferred_term = p.clone();
                        item.hit_count += 1;
                    } else {
                        list.push(GlossaryEntry {
                            spoken_variant: s.clone(),
                            preferred_term: p.clone(),
                            hit_count: 1,
                        });
                    }
                }
                state.notify(ToastSeverity::Success, format!("Added {s} → {p}"));
            }
        });
    };

    rsx! {
        div { class: "tc-insp-header",
            div { class: "tc-insp-header__title",
                span { "Personal Glossary" }
                span { class: "tc-insp-header__count tc-mono", "{entries.len()}" }
            }
            span { class: "tc-form-row__sub", "SQLite Local" }
        }

        div { class: "tc-insp-toolbar",
            div { class: "tc-search-wrap",
                span { class: "tc-search-wrap__icon", IconSearch {} }
                input {
                    class: "tc-search-input",
                    placeholder: "Filter glossary terms…",
                    value: "{search}",
                    onfocus: move |_| state.editing_text.set(true),
                    onblur: move |_| state.editing_text.set(false),
                    oninput: move |e| search.set(e.value()),
                }
            }
        }

        div { class: "tc-insp-body",
            div { class: "tc-insp-body--form",
                div { class: "tc-group",
                    div { class: "tc-group__title", "Teach New Term" }
                    div { class: "tc-group__card",
                        div { class: "tc-form-row tc-form-row--stack",
                            div { class: "tc-grid-2",
                                input {
                                    class: "tc-input",
                                    placeholder: "Spoken (e.g. cơ sơ)",
                                    value: "{spoken_in}",
                                    onfocus: move |_| state.editing_text.set(true),
                                    onblur: move |_| state.editing_text.set(false),
                                    oninput: move |e| spoken_in.set(e.value()),
                                }
                                input {
                                    class: "tc-input",
                                    placeholder: "Preferred (e.g. Cursor)",
                                    value: "{preferred_in}",
                                    onfocus: move |_| state.editing_text.set(true),
                                    onblur: move |_| state.editing_text.set(false),
                                    oninput: move |e| preferred_in.set(e.value()),
                                }
                            }
                            button {
                                class: "tc-btn",
                                onclick: on_add,
                                IconPlus {}
                                "Add to Glossary"
                            }
                        }
                    }
                }

                div { class: "tc-group",
                    div { class: "tc-group__title", "Whisper initial_prompt" }
                    div { class: "tc-prompt-box tc-mono", "{prompt_preview}" }
                }

                div { class: "tc-group__title",
                    span { "Saved Terms" }
                    span { class: "tc-group__sub tc-mono", "{filtered.len()}" }
                }
            }

            div { class: "tc-insp-body--list",
                for entry in filtered {
                    {
                        let spoken = entry.spoken_variant.clone();
                        rsx! {
                            div {
                                key: "gloss-{entry.spoken_variant}",
                                class: "tc-row",
                                div { class: "tc-flex-between",
                                    div { class: "tc-flex-row",
                                        span { class: "tc-muted", "{entry.spoken_variant}" }
                                        span { class: "tc-muted", "→" }
                                        span { "{entry.preferred_term}" }
                                    }
                                    div { class: "tc-flex-row tc-gap-1",
                                        span { class: "tc-form-row__sub tc-mono", "×{entry.hit_count}" }
                                        button {
                                            class: "tc-btn tc-btn--ghost tc-btn--sm tc-btn--icon",
                                            title: "Remove term",
                                            onclick: move |_| {
                                                let s = spoken.clone();
                                                spawn(async move {
                                                    if is_tauri_runtime() {
                                                        match cmd_delete_glossary_term(s.clone()).await {
                                                            Ok(list) => {
                                                                state.glossary.set(list);
                                                                state.notify(ToastSeverity::Info, format!("Removed '{s}'"));
                                                            }
                                                            Err(e) => state.notify(ToastSeverity::Error, format!("Delete failed: {e}")),
                                                        }
                                                    } else {
                                                        state.glossary.write().retain(|g| !g.spoken_variant.eq_ignore_ascii_case(&s));
                                                        state.notify(ToastSeverity::Info, format!("Removed '{s}'"));
                                                    }
                                                });
                                            },
                                            IconClose {}
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
