use std::path::Path;
use tauri::State;
use tinycut_ai_audio::LocalAudioEngine;
use tinycut_core_domain::{
    GlossaryEntry, MediaRef, RedactionTrack, ShieldConfig, Transcript,
};
use tinycut_export_engine::{
    export_ass, export_edl, export_fcpxml, export_premiere_xml, export_srt,
};
use tinycut_local_store::{GlossaryStore, SqliteStore};
use tinycut_media_engine::{FfmpegMediaExtractor, MediaExtractor};
use tinycut_sync_client::scrub_pii;
use tinycut_vision_shield::LocalVisionEngine;

pub struct AppState {
    pub store: SqliteStore,
    pub media: FfmpegMediaExtractor,
    pub audio: LocalAudioEngine,
    pub vision: LocalVisionEngine,
}

impl AppState {
    fn new() -> Self {
        let store = open_default_store().unwrap_or_else(|_| {
            SqliteStore::open_in_memory().expect("in-memory sqlite fallback must open")
        });
        Self {
            store,
            media: FfmpegMediaExtractor::default(),
            audio: LocalAudioEngine::default(),
            vision: LocalVisionEngine::new(),
        }
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
async fn transcribe_local(
    state: State<'_, AppState>,
    media: MediaRef,
) -> Result<Transcript, String> {
    let preferred = state
        .store
        .list_preferred_terms()
        .map_err(|e| e.to_string())?;
    let entries = state.store.all_entries().map_err(|e| e.to_string())?;
    let raw = state
        .audio
        .transcribe_local(&media, &preferred)
        .await
        .map_err(|e| e.to_string())?;
    state
        .audio
        .fix_subtitles_local(&raw, &entries)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn fix_subtitles_local(
    state: State<'_, AppState>,
    transcript: Transcript,
) -> Result<Transcript, String> {
    let entries = state.store.all_entries().map_err(|e| e.to_string())?;
    state
        .audio
        .fix_subtitles_local(&transcript, &entries)
        .await
        .map_err(|e| e.to_string())
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
fn detect_privacy_local(
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
    state
        .vision
        .detect_privacy_local(&media, &config, &whitelist)
        .map_err(|e| e.to_string())
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
            transcribe_local,
            fix_subtitles_local,
            upsert_glossary_term,
            list_glossary,
            export_subtitles,
            detect_privacy_local,
            scrub_text_preview,
        ])
        .run(tauri::generate_context!())
        .expect("error while running TinyCut");
}
