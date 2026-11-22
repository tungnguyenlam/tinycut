use tinycut_ai_audio::LocalAudioEngine;
use tinycut_core_domain::{MediaRef, Transcript};
use tinycut_export_engine::export_srt;

#[tauri::command]
async fn transcribe_local(media: MediaRef, glossary: Vec<String>) -> Result<Transcript, String> {
    let engine = LocalAudioEngine::new("whisper-v3-turbo-q5", "qwen3-1.7b-q4");
    engine
        .transcribe_local(&media, &glossary)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn transcript_to_srt(transcript: Transcript) -> String {
    export_srt(&transcript)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![transcribe_local, transcript_to_srt])
        .run(tauri::generate_context!())
        .expect("error while running TinyCut");
}
