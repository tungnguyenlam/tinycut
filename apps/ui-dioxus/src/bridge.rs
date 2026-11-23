use serde::{Serialize, de::DeserializeOwned};
use tinycut_core_domain::{
    EditDiff, GlossaryEntry, MediaRef, ModelInstallStatus, ModelType, Project, RedactionTrack,
    ShieldConfig, Transcript,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI_INTERNALS__"], js_name = invoke, catch)]
    async fn tauri_invoke_raw(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI_INTERNALS__"], js_name = convertFileSrc)]
    fn tauri_convert_file_src_raw(file_path: &str, protocol: &str) -> String;
}

pub fn is_tauri_runtime() -> bool {
    js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("__TAURI_INTERNALS__"))
        .map(|v| !v.is_undefined() && !v.is_null())
        .unwrap_or(false)
}

pub fn convert_file_src(path: &str) -> Option<String> {
    if !is_tauri_runtime() || path.trim().is_empty() {
        return None;
    }
    Some(tauri_convert_file_src_raw(path, "asset"))
}

pub async fn invoke_tauri<A: Serialize, R: DeserializeOwned>(
    cmd: &str,
    args: &A,
) -> Result<R, String> {
    let js_args = serde_wasm_bindgen::to_value(args).map_err(|e| e.to_string())?;
    let val = tauri_invoke_raw(cmd, js_args)
        .await
        .map_err(|e| format!("{e:?}"))?;
    serde_wasm_bindgen::from_value(val).map_err(|e| e.to_string())
}

// ============================================================================
// Typed Tauri Command Wrappers
// ============================================================================

pub async fn cmd_open_project_from_path(
    path: String,
    is_nda_private: bool,
) -> Result<Project, String> {
    #[derive(Serialize)]
    struct Args {
        path: String,
        is_nda_private: bool,
    }
    invoke_tauri(
        "open_project_from_path",
        &Args {
            path,
            is_nda_private,
        },
    )
    .await
}

pub async fn cmd_list_recent_projects() -> Result<Vec<Project>, String> {
    #[derive(Serialize)]
    struct Empty {}
    invoke_tauri("list_recent_projects", &Empty {}).await
}

pub async fn cmd_save_project(project: Project) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args {
        project: Project,
    }
    invoke_tauri("save_project", &Args { project }).await
}

pub async fn cmd_transcribe_local(media: MediaRef) -> Result<Transcript, String> {
    #[derive(Serialize)]
    struct Args {
        media: MediaRef,
    }
    invoke_tauri("transcribe_local", &Args { media }).await
}

pub async fn cmd_detect_privacy_local(
    media: MediaRef,
    config: ShieldConfig,
) -> Result<Vec<RedactionTrack>, String> {
    #[derive(Serialize)]
    struct Args {
        media: MediaRef,
        config: ShieldConfig,
    }
    invoke_tauri("detect_privacy_local", &Args { media, config }).await
}

pub async fn cmd_list_glossary() -> Result<Vec<GlossaryEntry>, String> {
    #[derive(Serialize)]
    struct Empty {}
    invoke_tauri("list_glossary", &Empty {}).await
}

pub async fn cmd_upsert_glossary_term(
    spoken_variant: String,
    preferred_term: String,
) -> Result<Vec<GlossaryEntry>, String> {
    #[derive(Serialize)]
    struct Args {
        spoken_variant: String,
        preferred_term: String,
    }
    invoke_tauri(
        "upsert_glossary_term",
        &Args {
            spoken_variant,
            preferred_term,
        },
    )
    .await
}

pub async fn cmd_delete_glossary_term(
    spoken_variant: String,
) -> Result<Vec<GlossaryEntry>, String> {
    #[derive(Serialize)]
    struct Args {
        spoken_variant: String,
    }
    invoke_tauri("delete_glossary_term", &Args { spoken_variant }).await
}

pub async fn cmd_export_subtitles(
    transcript: Transcript,
    media: MediaRef,
    format: String,
) -> Result<String, String> {
    #[derive(Serialize)]
    struct Args {
        transcript: Transcript,
        media: MediaRef,
        format: String,
    }
    invoke_tauri(
        "export_subtitles",
        &Args {
            transcript,
            media,
            format,
        },
    )
    .await
}

pub async fn cmd_save_text_file(path: String, contents: String) -> Result<String, String> {
    #[derive(Serialize)]
    struct Args {
        path: String,
        contents: String,
    }
    invoke_tauri("save_text_file", &Args { path, contents }).await
}

pub async fn cmd_whitelist_creator_face(
    id: String,
    label: String,
    embedding: Vec<f32>,
) -> Result<usize, String> {
    #[derive(Serialize)]
    struct Args {
        id: String,
        label: String,
        embedding: Vec<f32>,
    }
    invoke_tauri(
        "whitelist_creator_face",
        &Args {
            id,
            label,
            embedding,
        },
    )
    .await
}

pub async fn cmd_record_edit_diff(
    project: Project,
    diff: EditDiff,
    opt_in_telemetry: bool,
) -> Result<bool, String> {
    #[derive(Serialize)]
    struct Args {
        project: Project,
        diff: EditDiff,
        opt_in_telemetry: bool,
    }
    invoke_tauri(
        "record_edit_diff",
        &Args {
            project,
            diff,
            opt_in_telemetry,
        },
    )
    .await
}

pub async fn cmd_list_models() -> Result<Vec<ModelInstallStatus>, String> {
    #[derive(Serialize)]
    struct Empty {}
    invoke_tauri("list_models", &Empty {}).await
}

pub async fn cmd_download_model(task: ModelType) -> Result<Vec<ModelInstallStatus>, String> {
    #[derive(Serialize)]
    struct Args {
        task: ModelType,
    }
    invoke_tauri("download_model", &Args { task }).await
}

pub async fn cmd_download_all_default_models() -> Result<Vec<ModelInstallStatus>, String> {
    #[derive(Serialize)]
    struct Empty {}
    invoke_tauri("download_all_default_models", &Empty {}).await
}

pub async fn cmd_delete_model(task: ModelType) -> Result<Vec<ModelInstallStatus>, String> {
    #[derive(Serialize)]
    struct Args {
        task: ModelType,
    }
    invoke_tauri("delete_model", &Args { task }).await
}

// ============================================================================
// Browser / WebView Helpers (Theme, Clipboard & Download Fallback)
// ============================================================================

const THEME_STORAGE_KEY: &str = "tinycut-theme";

/// Returns `true` for Light theme, `false` for Dark theme.
/// Honors persisted user choice first, then OS `prefers-color-scheme`.
pub fn detect_initial_is_light() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    if let Ok(Some(storage)) = window.local_storage()
        && let Ok(Some(saved)) = storage.get_item(THEME_STORAGE_KEY)
    {
        return saved == "light";
    }
    window
        .match_media("(prefers-color-scheme: light)")
        .ok()
        .flatten()
        .map(|mql| mql.matches())
        .unwrap_or(false)
}

/// Applies `data-theme="light" | "dark"` directly on `<html>` so `body` and
/// overscroll regions stay synchronized, optionally persisting to `localStorage`.
pub fn apply_theme_to_document(is_light: bool, persist: bool) {
    let value = if is_light { "light" } else { "dark" };
    let Some(window) = web_sys::window() else {
        return;
    };
    if let Some(doc) = window.document()
        && let Some(root) = doc.document_element()
    {
        let _ = root.set_attribute("data-theme", value);
    }
    if persist
        && let Ok(Some(storage)) = window.local_storage()
    {
        let _ = storage.set_item(THEME_STORAGE_KEY, value);
    }
}

pub async fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or("no browser window")?;
    let clipboard = window.navigator().clipboard();
    wasm_bindgen_futures::JsFuture::from(clipboard.write_text(text))
        .await
        .map_err(|e| format!("{e:?}"))?;
    Ok(())
}

pub fn download_text_web(filename: &str, content: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or("no browser window")?;
    let document = window.document().ok_or("no document")?;
    let parts = js_sys::Array::new();
    parts.push(&JsValue::from_str(content));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type("text/plain;charset=utf-8");
    let blob =
        web_sys::Blob::new_with_str_sequence_and_options(&parts, &opts).map_err(|e| format!("{e:?}"))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(|e| format!("{e:?}"))?;
    let a = document
        .create_element("a")
        .map_err(|e| format!("{e:?}"))?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| "failed to cast anchor")?;
    a.set_href(&url);
    a.set_download(filename);
    a.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}
