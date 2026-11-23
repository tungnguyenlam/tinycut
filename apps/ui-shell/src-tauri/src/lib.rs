use std::path::Path;
use tauri::State;
use tinycut_ai_audio::LocalAudioEngine;
use tinycut_core_domain::{
    EditDiff, GlossaryEntry, InferenceMode, MediaRef, ModelInstallStatus, ModelType, Project,
    RedactionTrack, ShieldConfig, Transcript,
};
use tinycut_export_engine::{
    export_ass, export_edl, export_fcpxml, export_premiere_xml, export_srt,
};
use tinycut_local_store::{GlossaryStore, SqliteStore, WhitelistedFace};
use tinycut_media_engine::{FfmpegMediaExtractor, MediaExtractor};
use tinycut_model_hub::ModelManager;
use tinycut_sync_client::scrub_pii;
use tinycut_vision_shield::LocalVisionEngine;
use uuid::Uuid;

pub struct AppState {
    pub store: SqliteStore,
    pub media: FfmpegMediaExtractor,
    pub models: ModelManager,
}

impl AppState {
    fn new() -> Self {
        let store = open_default_store().unwrap_or_else(|_| {
            SqliteStore::open_in_memory().expect("in-memory sqlite fallback must open")
        });
        Self {
            store,
            media: FfmpegMediaExtractor::default(),
            models: ModelManager::default(),
        }
    }

    fn audio_engine(&self) -> LocalAudioEngine {
        LocalAudioEngine::from_model_manager(&self.models)
    }

    fn vision_engine(&self) -> LocalVisionEngine {
        LocalVisionEngine::from_model_manager(&self.models)
    }
}

fn open_default_store() -> Result<SqliteStore, String> {
    let base = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|e| e.to_string())?;
    let dir = Path::new(&base).join(".tinycut");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    SqliteStore::open(dir.join("tinycut.sqlite")).map_err(|e| e.to_string())
}

#[tauri::command]
fn probe_media(state: State<'_, AppState>, path: String) -> Result<MediaRef, String> {
    state
        .media
        .probe_media(Path::new(&path))
        .map_err(|e| e.to_string())
}

#[tauri::command]
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
fn list_recent_projects(state: State<'_, AppState>) -> Result<Vec<Project>, String> {
    state.store.list_projects().map_err(|e| e.to_string())
}

#[tauri::command]
fn save_project(state: State<'_, AppState>, project: Project) -> Result<(), String> {
    state
        .store
        .save_project(&project)
        .map_err(|e| e.to_string())
}

#[tauri::command]
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
fn list_glossary(state: State<'_, AppState>) -> Result<Vec<GlossaryEntry>, String> {
    state.store.all_entries().map_err(|e| e.to_string())
}

#[tauri::command]
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
fn list_models(state: State<'_, AppState>) -> Result<Vec<ModelInstallStatus>, String> {
    Ok(state.models.list_statuses())
}

#[tauri::command]
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
fn delete_model(
    state: State<'_, AppState>,
    task: ModelType,
) -> Result<Vec<ModelInstallStatus>, String> {
    state.models.delete_model(task).map_err(|e| e.to_string())?;
    Ok(state.models.list_statuses())
}

#[tauri::command]
fn whitelist_creator_face(
    state: State<'_, AppState>,
    id: String,
    label: String,
    embedding: Vec<f32>,
) -> Result<usize, String> {
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
        .map(|v| v.len())
        .map_err(|e| e.to_string())
}

#[tauri::command]
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
fn scrub_text_preview(text: String) -> String {
    scrub_pii(&text)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            probe_media,
            open_project_from_path,
            list_recent_projects,
            save_project,
            transcribe_local,
            fix_subtitles_local,
            upsert_glossary_term,
            delete_glossary_term,
            list_glossary,
            export_subtitles,
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
        .run(tauri::generate_context!())
        .expect("error while running TinyCut");
}
