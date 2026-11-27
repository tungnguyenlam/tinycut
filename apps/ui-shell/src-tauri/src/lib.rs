use std::path::{Path, PathBuf};
use tauri::{Manager, State};
use tinycut_ai_audio::LocalAudioEngine;
use tinycut_core_domain::{
    EditDiff, GlossaryEntry, InferenceMode, MediaRef, ModelInstallStatus, ModelType, Project,
    RedactionTrack, ShieldConfig, Transcript,
};
use tinycut_export_engine::{
    export_ass, export_edl, export_fcpxml, export_premiere_xml, export_srt,
};
use tinycut_gpu_compositor::{CompositorFrameTask, create_best_compositor};
use tinycut_local_store::{GlossaryStore, SqliteStore, WhitelistedFace};
use tinycut_media_engine::{FfmpegMediaExtractor, MediaExtractor};
use tinycut_model_hub::ModelManager;
use tinycut_sync_client::scrub_pii;
use tinycut_vision_shield::{
    LocalVisionEngine, apply_zero_leak_padding, interpolate_boxes_for_frame,
};
use uuid::Uuid;

pub struct AppState {
    pub store: SqliteStore,
    pub media: FfmpegMediaExtractor,
    pub models: ModelManager,
    pub proxy_cache_dir: PathBuf,
}

impl AppState {
    fn new(data_dir: Option<PathBuf>) -> Self {
        let base_dir = resolve_app_data_dir(data_dir);
        let store = open_store_in(&base_dir).unwrap_or_else(|_| {
            SqliteStore::open_in_memory().expect("in-memory sqlite fallback must open")
        });
        let models = if std::env::var("TINYCUT_MODELS_DIR")
            .ok()
            .is_some_and(|v| !v.trim().is_empty())
        {
            ModelManager::default()
        } else {
            ModelManager::with_dir(base_dir.join("models"))
        };
        let proxy_cache_dir = base_dir.join("proxies");
        Self {
            store,
            media: FfmpegMediaExtractor::default(),
            models,
            proxy_cache_dir,
        }
    }

    fn audio_engine(&self) -> LocalAudioEngine {
        LocalAudioEngine::from_model_manager(&self.models)
    }

    fn vision_engine(&self) -> LocalVisionEngine {
        LocalVisionEngine::from_model_manager(&self.models)
    }
}

fn resolve_app_data_dir(tauri_data_dir: Option<PathBuf>) -> PathBuf {
    if let Some(dir) = tauri_data_dir {
        return dir;
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".tinycut")
}

fn open_store_in(base_dir: &Path) -> Result<SqliteStore, String> {
    std::fs::create_dir_all(base_dir).map_err(|e| e.to_string())?;
    SqliteStore::open(base_dir.join("tinycut.sqlite")).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn probe_media(state: State<'_, AppState>, path: String) -> Result<MediaRef, String> {
    state
        .media
        .probe_media(Path::new(&path))
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn open_project_from_path(
    state: State<'_, AppState>,
    path: String,
    is_nda_private: bool,
) -> Result<Project, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("media path cannot be empty".into());
    }
    let path_obj = Path::new(trimmed);
    let media = state
        .media
        .probe_media(path_obj)
        .unwrap_or_else(|_| MediaRef {
            id: Uuid::new_v4(),
            path: trimmed.to_string(),
            duration_ms: 12_000,
            width: 1920,
            height: 1080,
            fps: 30.0,
        });
    let title = path_obj
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("Untitled Project")
        .to_string();
    let project = Project {
        id: Uuid::new_v4(),
        title,
        media,
        is_nda_private,
        inference_mode: InferenceMode::Local,
        locale: "vi-VN".into(),
        content_domain: Some("creator".into()),
    };
    state
        .store
        .save_project(&project)
        .map_err(|e| e.to_string())?;
    Ok(project)
}

#[tauri::command]
#[specta::specta]
fn list_recent_projects(state: State<'_, AppState>) -> Result<Vec<Project>, String> {
    state.store.list_projects().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn save_project(state: State<'_, AppState>, project: Project) -> Result<(), String> {
    state
        .store
        .save_project(&project)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
async fn prepare_preview_proxy(
    state: State<'_, AppState>,
    media: MediaRef,
) -> Result<String, String> {
    let extractor = state.media.clone();
    let cache_dir = state.proxy_cache_dir.clone();
    tokio::task::spawn_blocking(move || {
        extractor
            .build_preview_proxy(&media, &cache_dir)
            .map(|p| p.display().to_string())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("preview proxy worker failed: {e}"))?
}

#[tauri::command]
#[specta::specta]
async fn extract_waveform(
    state: State<'_, AppState>,
    media: MediaRef,
    buckets: u32,
) -> Result<Vec<f32>, String> {
    let extractor = state.media.clone();
    let count = (buckets as usize).clamp(16, 2048);
    tokio::task::spawn_blocking(move || {
        extractor
            .extract_waveform_peaks(&media, count)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("waveform worker failed: {e}"))?
}

#[tauri::command]
#[specta::specta]
async fn transcribe_local(
    state: State<'_, AppState>,
    media: MediaRef,
) -> Result<Transcript, String> {
    let preferred = state
        .store
        .list_preferred_terms()
        .map_err(|e| e.to_string())?;
    let entries = state.store.all_entries().map_err(|e| e.to_string())?;
    let audio = state.audio_engine();
    tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();
        let raw = rt
            .block_on(audio.transcribe_local(&media, &preferred))
            .map_err(|e| e.to_string())?;
        rt.block_on(audio.fix_subtitles_local(&raw, &entries))
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("transcription worker failed: {e}"))?
}

#[tauri::command]
#[specta::specta]
async fn fix_subtitles_local(
    state: State<'_, AppState>,
    transcript: Transcript,
) -> Result<Transcript, String> {
    let entries = state.store.all_entries().map_err(|e| e.to_string())?;
    let audio = state.audio_engine();
    tokio::task::spawn_blocking(move || {
        tokio::runtime::Handle::current()
            .block_on(audio.fix_subtitles_local(&transcript, &entries))
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("subtitle fixer worker failed: {e}"))?
}

#[tauri::command]
#[specta::specta]
fn upsert_glossary_term(
    state: State<'_, AppState>,
    spoken_variant: String,
    preferred_term: String,
) -> Result<Vec<GlossaryEntry>, String> {
    state
        .store
        .upsert_glossary_term(&spoken_variant, &preferred_term)
        .map_err(|e| e.to_string())?;
    state.store.all_entries().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn delete_glossary_term(
    state: State<'_, AppState>,
    spoken_variant: String,
) -> Result<Vec<GlossaryEntry>, String> {
    state
        .store
        .delete_glossary_term(&spoken_variant)
        .map_err(|e| e.to_string())?;
    state.store.all_entries().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn list_glossary(state: State<'_, AppState>) -> Result<Vec<GlossaryEntry>, String> {
    state.store.all_entries().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn export_subtitles(
    transcript: Transcript,
    media: MediaRef,
    format: String,
) -> Result<String, String> {
    match format.as_str() {
        "srt" => Ok(export_srt(&transcript)),
        "ass" => Ok(export_ass(&transcript)),
        "fcpxml" => Ok(export_fcpxml(&transcript, &media)),
        "premiere_xml" => Ok(export_premiere_xml(&transcript, &media)),
        "edl" => Ok(export_edl(&transcript, media.fps.max(24.0))),
        other => Err(format!("unsupported export format: {other}")),
    }
}

#[tauri::command]
#[specta::specta]
async fn export_redacted_video(
    state: State<'_, AppState>,
    media: MediaRef,
    tracks: Vec<RedactionTrack>,
    config: ShieldConfig,
    output_path: String,
) -> Result<String, String> {
    let trimmed = output_path.trim().to_string();
    if trimmed.is_empty() {
        return Err("output path cannot be empty".into());
    }
    let extractor = state.media.clone();
    tokio::task::spawn_blocking(move || {
        let total_frames = ((media.duration_ms as f64 / 1000.0) * media.fps.max(1.0) as f64)
            .round()
            .max(1.0) as u64;
        let padded_tracks: Vec<RedactionTrack> = tracks
            .iter()
            .map(|t| apply_zero_leak_padding(t, &config, total_frames))
            .collect();

        let stream = extractor
            .stream_all_frames_rgba(&media)
            .map_err(|e| e.to_string())?;
        let width = stream.width();
        let height = stream.height();
        let fps = media.fps.max(1.0);
        let out_path = PathBuf::from(&trimmed);
        let audio_src = Path::new(&media.path);
        let mut writer = extractor
            .open_video_writer(width, height, fps, Some(audio_src), &out_path)
            .map_err(|e| e.to_string())?;

        let compositor = create_best_compositor();
        for frame_res in stream {
            let frame = frame_res.map_err(|e| e.to_string())?;
            let boxes = interpolate_boxes_for_frame(&padded_tracks, frame.frame_index);
            if boxes.is_empty() {
                writer
                    .write_frame_rgba(&frame.rgba)
                    .map_err(|e| e.to_string())?;
            } else {
                let task = CompositorFrameTask {
                    frame_index: frame.frame_index,
                    style: config.blur_style,
                    boxes,
                };
                let rendered = compositor
                    .render_frame_rgba8(frame.width, frame.height, &frame.rgba, &task)
                    .map_err(|e| e.to_string())?;
                writer
                    .write_frame_rgba(&rendered)
                    .map_err(|e| e.to_string())?;
            }
        }
        writer.finish().map_err(|e| e.to_string())?;
        Ok(out_path.display().to_string())
    })
    .await
    .map_err(|e| format!("burn-in video export worker failed: {e}"))?
}

#[tauri::command]
#[specta::specta]
fn save_text_file(path: String, contents: String) -> Result<String, String> {
    let target = Path::new(path.trim());
    if let Some(parent) = target.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(target, contents).map_err(|e| e.to_string())?;
    Ok(target.display().to_string())
}

#[tauri::command]
#[specta::specta]
async fn detect_privacy_local(
    state: State<'_, AppState>,
    media: MediaRef,
    config: ShieldConfig,
) -> Result<Vec<RedactionTrack>, String> {
    let whitelist: Vec<Vec<f32>> = state
        .store
        .list_face_whitelist()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|f| f.embedding)
        .collect();
    let vision = state.vision_engine();
    tokio::task::spawn_blocking(move || vision.detect_privacy_local(&media, &config, &whitelist))
        .await
        .map_err(|e| format!("privacy scan worker failed: {e}"))?
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn list_models(state: State<'_, AppState>) -> Result<Vec<ModelInstallStatus>, String> {
    Ok(state.models.list_statuses())
}

#[tauri::command]
#[specta::specta]
async fn download_model(
    state: State<'_, AppState>,
    task: ModelType,
) -> Result<Vec<ModelInstallStatus>, String> {
    state
        .models
        .ensure_task_bundle(task)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
async fn download_all_default_models(
    state: State<'_, AppState>,
) -> Result<Vec<ModelInstallStatus>, String> {
    state
        .models
        .ensure_all_defaults()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn delete_model(
    state: State<'_, AppState>,
    task: ModelType,
) -> Result<Vec<ModelInstallStatus>, String> {
    state.models.delete_model(task).map_err(|e| e.to_string())?;
    Ok(state.models.list_statuses())
}

#[tauri::command]
#[specta::specta]
fn whitelist_creator_face(
    state: State<'_, AppState>,
    id: String,
    label: String,
    embedding: Vec<f32>,
) -> Result<u32, String> {
    state
        .store
        .upsert_face_whitelist(&WhitelistedFace {
            id,
            label,
            embedding,
        })
        .map_err(|e| e.to_string())?;
    state
        .store
        .list_face_whitelist()
        .map(|v| v.len() as u32)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn record_edit_diff(
    state: State<'_, AppState>,
    project: Project,
    diff: EditDiff,
    opt_in_telemetry: bool,
) -> Result<bool, String> {
    if !opt_in_telemetry {
        return Ok(false);
    }
    // Enforce NDA lock server-side in Rust so NDA projects can never enqueue telemetry.
    project
        .allow_cloud_or_telemetry()
        .map_err(|e| e.to_string())?;
    let scrubbed = EditDiff {
        raw_asr: scrub_pii(&diff.raw_asr),
        llm_draft: diff.llm_draft.map(|d| scrub_pii(&d)),
        user_final: scrub_pii(&diff.user_final),
        edit_type: diff.edit_type,
        audio_clip_ref: None,
    };
    state
        .store
        .enqueue_text_diff(&scrubbed)
        .map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
#[specta::specta]
fn scrub_text_preview(text: String) -> String {
    scrub_pii(&text)
}

pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .disable_serde_phases()
        .dangerously_cast_bigints_to_number()
        .semantic_types(
            specta_typescript::semantic::Configuration::empty().enable_lossless_floats(),
        )
        .commands(tauri_specta::collect_commands![
        probe_media,
        open_project_from_path,
        list_recent_projects,
        save_project,
        prepare_preview_proxy,
        extract_waveform,
        transcribe_local,
        fix_subtitles_local,
        upsert_glossary_term,
        delete_glossary_term,
        list_glossary,
        export_subtitles,
        export_redacted_video,
        save_text_file,
        detect_privacy_local,
        list_models,
        download_model,
        download_all_default_models,
        delete_model,
        whitelist_creator_face,
        record_edit_diff,
        scrub_text_preview,
    ])
}

pub fn export_typescript_bindings(path: &Path) -> Result<(), String> {
    specta_builder()
        .export(specta_typescript::Typescript::default(), path)
        .map_err(|e| format!("failed to export TypeScript bindings: {e}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = specta_builder();

    #[cfg(debug_assertions)]
    {
        let bindings_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../ui-web/src/bindings.ts");
        if bindings_path.parent().is_some_and(|p| p.exists()) {
            let _ = export_typescript_bindings(&bindings_path);
        }
    }

    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir().ok();
            app.manage(AppState::new(data_dir));
            Ok(())
        })
        .invoke_handler(builder.invoke_handler())
        .run(tauri::generate_context!())
        .expect("error while running TinyCut");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_valid_typescript_bindings() {
        let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui-web/src/bindings.ts");
        export_typescript_bindings(&out).expect("tauri-specta TypeScript export must succeed");
        let content = std::fs::read_to_string(&out).expect("bindings file must be readable");
        assert!(content.contains("preparePreviewProxy") || content.contains("prepare_preview_proxy"));
        assert!(content.contains("exportRedactedVideo") || content.contains("export_redacted_video"));
    }
}
