use crate::bridge::{
    cmd_delete_model, cmd_download_all_default_models, cmd_download_model,
    cmd_open_project_from_path, is_tauri_runtime,
};
use crate::components::icons::{IconLock, IconUpload};
use crate::domain_ext::format_seg_time;
use crate::state::{StudioMode, StudioState, ToastSeverity, sample_media};
use dioxus::prelude::*;
use tinycut_core_domain::{InferenceMode, Project};
use uuid::Uuid;

fn format_model_size(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.0} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{bytes} B")
    }
}

#[component]
pub fn MediaInspector() -> Element {
    let mut state = use_context::<StudioState>();
    let mut path_input = use_signal(|| state.project.read().media.path.clone());

    let project = state.project.read().clone();
    let recent = state.recent_projects.read().clone();
    let models = state.models.read().clone();
    let installed_count = models.iter().filter(|m| m.installed).count();
    let busy = state.busy_job.read().is_some();
    let opt_in = *state.opt_in_telemetry.read();

    let on_open_path = move |_| {
        let p = path_input.read().trim().to_string();
        if p.is_empty() {
            state.notify(ToastSeverity::Warning, "Enter a local video or audio path");
            return;
        }
        let nda = state.project.read().is_nda_private;
        spawn(async move {
            if is_tauri_runtime() {
                match cmd_open_project_from_path(p.clone(), nda).await {
                    Ok(proj) => {
                        state.project.set(proj.clone());
                        state.is_demo_media.set(false);
                        state.current_ms.set(0);
                        state.playing.set(false);
                        {
                            let mut rec = state.recent_projects.write();
                            rec.retain(|x| x.id != proj.id);
                            rec.insert(0, proj.clone());
                        }
                        state.notify(
                            ToastSeverity::Success,
                            format!(
                                "Opened {} ({}×{} · {:.0}fps)",
                                proj.title, proj.media.width, proj.media.height, proj.media.fps
                            ),
                        );
                        state.mode.set(StudioMode::Subtitles);
                    }
                    Err(e) => state.notify(ToastSeverity::Error, format!("Open failed: {e}")),
                }
            } else {
                let title = p
                    .rsplit('/')
                    .next()
                    .unwrap_or("project")
                    .split('.')
                    .next()
                    .unwrap_or("project")
                    .to_string();
                let proj = Project {
                    id: Uuid::new_v4(),
                    title: title.clone(),
                    media: sample_media(&p),
                    is_nda_private: nda,
                    inference_mode: InferenceMode::Local,
                    locale: "vi-VN".into(),
                    content_domain: Some("creator".into()),
                };
                state.project.set(proj.clone());
                state.recent_projects.write().insert(0, proj);
                state.notify(ToastSeverity::Info, format!("Loaded '{title}' (preview)"));
            }
        });
    };

    rsx! {
        div { class: "tc-insp-header",
            div { class: "tc-insp-header__title",
                span { "Media & Projects" }
            }
            button {
                class: "tc-btn tc-btn--sm",
                onclick: move |_| state.load_demo(),
                "Load Demo"
            }
        }

        div { class: "tc-insp-body",
            div { class: "tc-insp-body--form",
                div { class: "tc-dropzone",
                    div { class: "tc-dropzone__icon", IconUpload {} }
                    div { class: "tc-dropzone__title", "Open Local Video or Audio" }
                    div { class: "tc-dropzone__hint", "MP4, MOV, MKV, WAV, M4A · raw 4K stays local" }
                    div { class: "tc-flex-row tc-w-full",
                        input {
                            class: "tc-input tc-mono tc-grow",
                            placeholder: "/path/to/street_vlog_4k.mp4",
                            value: "{path_input}",
                            onfocus: move |_| state.editing_text.set(true),
                            onblur: move |_| state.editing_text.set(false),
                            oninput: move |e| path_input.set(e.value()),
                        }
                        button {
                            class: "tc-btn",
                            onclick: on_open_path,
                            "Open"
                        }
                    }
                }

                // Active Media Inset Form Group
                div { class: "tc-group",
                    div { class: "tc-group__title", "Active Media" }
                    div { class: "tc-group__card",
                        div { class: "tc-form-row",
                            span { class: "tc-form-row__label", "Title" }
                            span { class: "tc-form-row__sub", "{project.title}" }
                        }
                        div { class: "tc-form-row",
                            span { class: "tc-form-row__label", "Resolution" }
                            span { class: "tc-form-row__sub tc-mono", "{project.media.width}×{project.media.height} · {project.media.fps:.0}fps" }
                        }
                        div { class: "tc-form-row",
                            span { class: "tc-form-row__label", "Duration" }
                            span { class: "tc-form-row__sub tc-mono", "{format_seg_time(project.media.duration_ms)}" }
                        }
                        div { class: "tc-form-row",
                            span { class: "tc-form-row__label", "Locale" }
                            span { class: "tc-form-row__sub tc-mono", "{project.locale}" }
                        }
                    }
                }

                // Local Lightweight AI Models Inset Form Group
                div { class: "tc-group",
                    div { class: "tc-group__title",
                        span { "Default Lightweight Models" }
                        div { class: "tc-flex-row",
                            span { class: "tc-group__sub tc-mono", "{installed_count}/{models.len()}" }
                            if installed_count < models.len() {
                                button {
                                    class: "tc-btn tc-btn--sm",
                                    disabled: busy,
                                    onclick: move |_| {
                                        state.busy_job.set(Some("Downloading default models".into()));
                                        spawn(async move {
                                            if is_tauri_runtime() {
                                                match cmd_download_all_default_models().await {
                                                    Ok(updated) => {
                                                        state.models.set(updated);
                                                        state.notify(
                                                            ToastSeverity::Success,
                                                            "All default lightweight models downloaded",
                                                        );
                                                    }
                                                    Err(e) => state.notify(
                                                        ToastSeverity::Error,
                                                        format!("Model download failed: {e}"),
                                                    ),
                                                }
                                            } else {
                                                for m in state.models.write().iter_mut() {
                                                    m.installed = true;
                                                    m.bytes_on_disk = m.spec.size_bytes;
                                                }
                                                state.notify(
                                                    ToastSeverity::Success,
                                                    "Preview: marked all default models ready",
                                                );
                                            }
                                            state.busy_job.set(None);
                                        });
                                    },
                                    if busy { "Downloading…" } else { "Download All" }
                                }
                            }
                        }
                    }
                    div { class: "tc-group__card",
                        for model in models.iter().cloned() {
                            {
                                let task = model.spec.task;
                                let display_name = model.spec.display_name.clone();
                                let size_str = format_model_size(if model.installed {
                                    model.bytes_on_disk
                                } else {
                                    model.spec.size_bytes
                                });
                                rsx! {
                                    div { key: "mdl-{model.spec.id}", class: "tc-form-row",
                                        div {
                                            div { class: "tc-form-row__label", "{model.spec.task_label}" }
                                            div { class: "tc-form-row__sub tc-mono",
                                                "{model.spec.display_name} · {size_str} · {model.spec.license}"
                                            }
                                        }
                                        if model.installed {
                                            button {
                                                class: "tc-btn tc-btn--ghost tc-btn--sm",
                                                disabled: busy,
                                                title: "Remove cached model file",
                                                onclick: move |_| {
                                                    let name = display_name.clone();
                                                    spawn(async move {
                                                        if is_tauri_runtime() {
                                                            match cmd_delete_model(task).await {
                                                                Ok(updated) => {
                                                                    state.models.set(updated);
                                                                    state.notify(
                                                                        ToastSeverity::Info,
                                                                        format!("Removed {name} from local cache"),
                                                                    );
                                                                }
                                                                Err(e) => state.notify(
                                                                    ToastSeverity::Error,
                                                                    format!("Remove failed: {e}"),
                                                                ),
                                                            }
                                                        } else {
                                                            for m in state.models.write().iter_mut() {
                                                                if m.spec.task == task {
                                                                    m.installed = false;
                                                                    m.bytes_on_disk = 0;
                                                                }
                                                            }
                                                        }
                                                    });
                                                },
                                                "Ready"
                                            }
                                        } else {
                                            button {
                                                class: "tc-btn tc-btn--sm",
                                                disabled: busy,
                                                onclick: move |_| {
                                                    let name = display_name.clone();
                                                    state.busy_job.set(Some(format!("Downloading {name}")));
                                                    spawn(async move {
                                                        if is_tauri_runtime() {
                                                            match cmd_download_model(task).await {
                                                                Ok(updated) => {
                                                                    state.models.set(updated);
                                                                    state.notify(
                                                                        ToastSeverity::Success,
                                                                        format!("Downloaded {name}"),
                                                                    );
                                                                }
                                                                Err(e) => state.notify(
                                                                    ToastSeverity::Error,
                                                                    format!("Download failed: {e}"),
                                                                ),
                                                            }
                                                        } else {
                                                            for m in state.models.write().iter_mut() {
                                                                if m.spec.task == task {
                                                                    m.installed = true;
                                                                    m.bytes_on_disk = m.spec.size_bytes;
                                                                }
                                                            }
                                                            state.notify(
                                                                ToastSeverity::Success,
                                                                format!("Preview: {name} ready"),
                                                            );
                                                        }
                                                        state.busy_job.set(None);
                                                    });
                                                },
                                                "Download"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        div { class: "tc-form-row",
                            div {
                                div { class: "tc-form-row__label", "QR Code Detection" }
                                div { class: "tc-form-row__sub tc-mono", "rqrr (Pure Rust) · 0 MB · MIT/Apache-2.0" }
                            }
                            span { class: "tc-form-row__sub tc-mono", "Built-in" }
                        }
                    }
                }

                // Privacy Firewall Inset Form Group
                div { class: "tc-group",
                    div { class: "tc-group__title",
                        span { "Privacy Firewall" }
                        if project.is_nda_private {
                            span { class: "tc-privacy-btn__nda",
                                IconLock {}
                                "NDA Locked"
                            }
                        } else {
                            span { class: "tc-group__sub", "Local Default" }
                        }
                    }
                    div { class: "tc-group__card",
                        div { class: "tc-form-row",
                            div {
                                div { class: "tc-form-row__label", "Anonymized diffs" }
                                div { class: "tc-form-row__sub", "PII scrubbed on-device; never sends faces/plates" }
                            }
                            button {
                                class: if opt_in && !project.is_nda_private { "tc-switch tc-switch--on" } else { "tc-switch" },
                                disabled: project.is_nda_private,
                                role: "switch",
                                "aria-checked": "{opt_in && !project.is_nda_private}",
                                onclick: move |_| {
                                    let next = !*state.opt_in_telemetry.read();
                                    state.opt_in_telemetry.set(next);
                                    if next {
                                        state.notify(ToastSeverity::Info, "Opt-in anonymized text diffs enabled");
                                    } else {
                                        state.notify(ToastSeverity::Info, "Telemetry disabled — 100% offline");
                                    }
                                },
                            }
                        }
                    }
                }

                div { class: "tc-group__title",
                    span { "Recent Projects" }
                    span { class: "tc-group__sub tc-mono", "{recent.len()}" }
                }
            }

            div { class: "tc-insp-body--list",
                for proj in recent {
                    {
                        let is_cur = proj.id == project.id;
                        rsx! {
                            div {
                                key: "rec-{proj.id}",
                                class: if is_cur { "tc-row tc-row--active" } else { "tc-row" },
                                onclick: move |_| {
                                    path_input.set(proj.media.path.clone());
                                    state.project.set(proj.clone());
                                    state.notify(ToastSeverity::Info, format!("Switched to {}", proj.title));
                                },
                                div { class: "tc-flex-between",
                                    div {
                                        div { "{proj.title}" }
                                        div { class: "tc-form-row__sub tc-mono",
                                            "{proj.media.width}×{proj.media.height} · {format_seg_time(proj.media.duration_ms)}"
                                        }
                                    }
                                    if proj.is_nda_private {
                                        span { class: "tc-privacy-btn__nda", IconLock {} }
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
