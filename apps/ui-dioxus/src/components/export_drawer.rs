use crate::bridge::{
    cmd_export_subtitles, cmd_save_text_file, copy_to_clipboard, download_text_web,
    is_tauri_runtime,
};
use crate::components::icons::{IconClose, IconExport};
use crate::domain_ext::{prepare_export_transcript, render_export_local};
use crate::state::{ExportFormat, StudioState, ToastSeverity};
use dioxus::prelude::*;

#[component]
pub fn ExportDrawer() -> Element {
    let mut state = use_context::<StudioState>();
    if !*state.show_export_drawer.read() {
        return rsx! {};
    }

    let format = *state.export_format.read();
    let strip_fillers = *state.export_strip_fillers.read();
    let include_speaker = *state.export_include_speaker.read();
    let max_chars = *state.export_max_chars.read();
    let copied = *state.export_copied.read();
    let project = state.project.read().clone();
    let transcript = state.transcript.read().clone();

    let prepared = prepare_export_transcript(
        &transcript,
        strip_fillers,
        include_speaker,
        max_chars,
    );
    let preview_text = render_export_local(&prepared, &project.media, format.id());

    let suggested_filename = format!("{}.{}", project.title, format.extension());
    let mut save_path = use_signal(move || format!("./{suggested_filename}"));

    let on_copy = {
        let text = preview_text.clone();
        move |_| {
            let text = text.clone();
            spawn(async move {
                match copy_to_clipboard(&text).await {
                    Ok(()) => {
                        state.export_copied.set(true);
                        state.notify(
                            ToastSeverity::Success,
                            format!("Copied {} to clipboard", format.label()),
                        );
                    }
                    Err(e) => state.notify(ToastSeverity::Error, format!("Copy failed: {e}")),
                }
            });
        }
    };

    let on_save = {
        let prepared = prepared.clone();
        let media = project.media.clone();
        let fallback_text = preview_text.clone();
        move |_| {
            let prepared = prepared.clone();
            let media = media.clone();
            let fallback_text = fallback_text.clone();
            let target_path = save_path.read().trim().to_string();
            let fname = format!("{}.{}", state.project.read().title, format.extension());
            spawn(async move {
                if is_tauri_runtime() {
                    let content = match cmd_export_subtitles(prepared, media, format.id().into()).await {
                        Ok(c) => c,
                        Err(_) => fallback_text,
                    };
                    match cmd_save_text_file(target_path, content).await {
                        Ok(saved) => state.notify(
                            ToastSeverity::Success,
                            format!("Exported {} → {saved}", format.label()),
                        ),
                        Err(e) => state.notify(ToastSeverity::Error, format!("Save failed: {e}")),
                    }
                } else {
                    match download_text_web(&fname, &fallback_text) {
                        Ok(()) => state.notify(
                            ToastSeverity::Success,
                            format!("Downloaded {fname}"),
                        ),
                        Err(e) => state.notify(ToastSeverity::Error, format!("Download failed: {e}")),
                    }
                }
            });
        }
    };

    let active_masks = state
        .tracks
        .read()
        .iter()
        .filter(|t| t.enabled && !t.whitelisted)
        .count();

    let lines: Vec<String> = preview_text.lines().map(|l| l.to_string()).collect();

    rsx! {
        div {
            class: "tc-backdrop",
            onclick: move |_| state.show_export_drawer.set(false),
        }

        div { class: "tc-sheet tc-glass-thick", role: "dialog", "aria-label": "Export Studio Output",
            div { class: "tc-sheet__head",
                div {
                    div { class: "tc-sheet__title", "Export" }
                    div { class: "tc-sheet__sub",
                        "{project.title} · {transcript.segments.len()} subtitles · {active_masks} active masks"
                    }
                }
                button {
                    class: "tc-btn tc-btn--ghost tc-btn--icon",
                    onclick: move |_| state.show_export_drawer.set(false),
                    IconClose {}
                }
            }

            div { class: "tc-sheet__body",
                // Format Radio List
                div { class: "tc-group",
                    div { class: "tc-group__title", "Format" }
                    div { class: "tc-radio-list",
                        for fmt in ExportFormat::ALL {
                            button {
                                key: "{fmt.id()}",
                                class: if format == fmt {
                                    "tc-radio-row tc-radio-row--active"
                                } else {
                                    "tc-radio-row"
                                },
                                onclick: move |_| {
                                    state.export_format.set(fmt);
                                    state.export_copied.set(false);
                                    save_path.set(format!("./{}.{}", state.project.read().title, fmt.extension()));
                                },
                                div { class: "tc-flex-row",
                                    span { class: "tc-radio-dot" }
                                    span { "{fmt.label()}" }
                                }
                                span { class: "tc-form-row__sub", "{fmt.hint()}" }
                            }
                        }
                    }
                }

                // Formatting Options Group
                div { class: "tc-group",
                    div { class: "tc-group__title", "Options" }
                    div { class: "tc-group__card",
                        div { class: "tc-form-row",
                            span { class: "tc-form-row__label", "Strip speech fillers" }
                            button {
                                class: if strip_fillers { "tc-switch tc-switch--on" } else { "tc-switch" },
                                role: "switch",
                                "aria-checked": "{strip_fillers}",
                                onclick: move |_| state.export_strip_fillers.set(!strip_fillers),
                            }
                        }
                        div { class: "tc-form-row",
                            span { class: "tc-form-row__label", "Include speaker label" }
                            button {
                                class: if include_speaker { "tc-switch tc-switch--on" } else { "tc-switch" },
                                role: "switch",
                                "aria-checked": "{include_speaker}",
                                onclick: move |_| state.export_include_speaker.set(!include_speaker),
                            }
                        }
                        div { class: "tc-form-row",
                            span { class: "tc-form-row__label", "Line wrap width" }
                            div { class: "tc-stepper",
                                button {
                                    class: "tc-stepper__btn",
                                    onclick: move |_| {
                                        let cur = *state.export_max_chars.read();
                                        state.export_max_chars.set(cur.saturating_sub(4).max(24));
                                    },
                                    "−"
                                }
                                span { class: "tc-stepper__val tc-mono", "{max_chars} ch" }
                                button {
                                    class: "tc-stepper__btn",
                                    onclick: move |_| {
                                        let cur = *state.export_max_chars.read();
                                        state.export_max_chars.set((cur + 4).min(80));
                                    },
                                    "+"
                                }
                            }
                        }
                    }
                }

                if is_tauri_runtime() {
                    div { class: "tc-group",
                        div { class: "tc-group__title", "Destination Path" }
                        input {
                            class: "tc-input tc-mono",
                            value: "{save_path}",
                            onfocus: move |_| state.editing_text.set(true),
                            onblur: move |_| state.editing_text.set(false),
                            oninput: move |e| save_path.set(e.value()),
                        }
                    }
                }

                // Live Preview
                div { class: "tc-group",
                    div { class: "tc-group__title",
                        span { "Preview ({format.label()})" }
                        span { class: "tc-group__sub tc-mono", "{lines.len()} lines" }
                    }
                    div { class: "tc-code-box",
                        for (i, line) in lines.iter().enumerate() {
                            {
                                let line_cls = if line.contains("-->") || line.starts_with("Dialogue:") || line.starts_with("00") {
                                    "tc-code-line__text tc-code-line__text--tc"
                                } else if line.trim_start().starts_with('<') || line.starts_with('[') {
                                    "tc-code-line__text tc-code-line__text--tag"
                                } else {
                                    "tc-code-line__text"
                                };
                                rsx! {
                                    div { key: "exp-l-{i}", class: "tc-code-line",
                                        span { class: "tc-code-line__num", "{i + 1}" }
                                        span { class: "{line_cls}", "{line}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            div { class: "tc-sheet__foot",
                button {
                    class: "tc-btn",
                    onclick: on_copy,
                    if copied { "Copied" } else { "Copy" }
                }
                button {
                    class: "tc-btn tc-btn--primary",
                    onclick: on_save,
                    IconExport {}
                    if is_tauri_runtime() { "Save to Disk" } else { "Download .{format.extension()}" }
                }
            }
        }
    }
}
