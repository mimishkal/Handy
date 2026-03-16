use crate::managers::history::HistoryManager;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{get_settings, write_settings, ModelUnloadTimeout};
use log::info;
use serde::Serialize;
use specta::Type;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

#[derive(Serialize, Type)]
pub struct ModelLoadStatus {
    is_loaded: bool,
    current_model: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub fn set_model_unload_timeout(app: AppHandle, timeout: ModelUnloadTimeout) {
    let mut settings = get_settings(&app);
    settings.model_unload_timeout = timeout;
    write_settings(&app, settings);
}

#[tauri::command]
#[specta::specta]
pub fn set_file_transcribe_chunking(app: AppHandle, enabled: bool) {
    let mut settings = get_settings(&app);
    settings.file_transcribe_chunking = enabled;
    write_settings(&app, settings);
}

#[tauri::command]
#[specta::specta]
pub fn set_file_transcribe_chunk_seconds(app: AppHandle, seconds: u32) {
    let mut settings = get_settings(&app);
    settings.file_transcribe_chunk_seconds = seconds.max(10);
    write_settings(&app, settings);
}

#[tauri::command]
#[specta::specta]
pub fn get_model_load_status(
    transcription_manager: State<TranscriptionManager>,
) -> Result<ModelLoadStatus, String> {
    Ok(ModelLoadStatus {
        is_loaded: transcription_manager.is_model_loaded(),
        current_model: transcription_manager.get_current_model(),
    })
}

#[tauri::command]
#[specta::specta]
pub fn unload_model_manually(
    transcription_manager: State<TranscriptionManager>,
) -> Result<(), String> {
    transcription_manager
        .unload_model()
        .map_err(|e| format!("Failed to unload model: {}", e))
}

#[derive(Clone, Serialize, Type)]
pub struct TranscribeFileProgress {
    pub step: String,
    pub current_chunk: u32,
    pub total_chunks: u32,
}

fn emit_progress(app: &AppHandle, step: &str, current_chunk: u32, total_chunks: u32) {
    let _ = app.emit(
        "transcribe-file-progress",
        TranscribeFileProgress {
            step: step.to_string(),
            current_chunk,
            total_chunks,
        },
    );
}

/// Overlap: 2 seconds at 16kHz — prevents words on chunk boundaries from being cut
const OVERLAP_SAMPLES: usize = 2 * 16_000;

/// Split samples into overlapping chunks and return (start, end) ranges
fn build_chunks(total_len: usize, chunk_seconds: u32) -> Vec<(usize, usize)> {
    let chunk_samples = chunk_seconds as usize * 16_000;
    if total_len <= chunk_samples {
        return vec![(0, total_len)];
    }
    let step = chunk_samples - OVERLAP_SAMPLES;
    let mut ranges = Vec::new();
    let mut start = 0;
    while start < total_len {
        let end = (start + chunk_samples).min(total_len);
        ranges.push((start, end));
        if end == total_len {
            break;
        }
        start += step;
    }
    ranges
}

#[tauri::command]
#[specta::specta]
pub async fn transcribe_file(
    app: AppHandle,
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
    history_manager: State<'_, Arc<HistoryManager>>,
    file_path: String,
) -> Result<String, String> {
    info!("Transcribing file: {}", file_path);

    // Step 1: Read and decode audio file
    emit_progress(&app, "reading", 0, 0);
    let samples = crate::audio_toolkit::read_audio_file(&file_path)
        .map_err(|e| format!("Failed to read audio file: {}", e))?;

    if samples.is_empty() {
        return Err("Audio file is empty or contains no audio data".to_string());
    }

    // Step 2: Ensure model is loaded
    emit_progress(&app, "loading_model", 0, 0);
    transcription_manager.initiate_model_load();

    // Step 3: Transcribe (with optional chunking)
    let settings = crate::settings::get_settings(&app);
    let text = if settings.file_transcribe_chunking {
        let chunk_ranges = build_chunks(samples.len(), settings.file_transcribe_chunk_seconds);
        let total_chunks = chunk_ranges.len() as u32;
        let mut results: Vec<String> = Vec::new();

        info!(
            "Transcribing {} chunks ({:.1}s audio, {}s per chunk)",
            total_chunks,
            samples.len() as f64 / 16000.0,
            settings.file_transcribe_chunk_seconds
        );

        for (i, (start, end)) in chunk_ranges.iter().enumerate() {
            emit_progress(&app, "transcribing", (i + 1) as u32, total_chunks);
            let text = transcription_manager
                .transcribe(samples[*start..*end].to_vec())
                .map_err(|e| format!("Transcription failed on chunk {}: {}", i + 1, e))?;
            if !text.is_empty() {
                results.push(text);
            }
        }
        results.join(" ")
    } else {
        emit_progress(&app, "transcribing", 1, 1);
        transcription_manager
            .transcribe(samples.clone())
            .map_err(|e| format!("Transcription failed: {}", e))?
    };
    info!("Transcription result: {}", text);

    // Step 4: Save to history
    emit_progress(&app, "saving", 0, 0);
    history_manager
        .save_transcription(samples, text.clone(), None, None)
        .await
        .map_err(|e| format!("Failed to save to history: {}", e))?;

    Ok(text)
}
