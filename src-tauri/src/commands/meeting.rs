use crate::managers::meeting::{MeetingManager, MeetingSegment, MeetingSummary};
use std::sync::Arc;
use tauri::{AppHandle, State};

/// Check if Screen Recording permission is granted (macOS only).
/// Returns true on non-macOS platforms (not needed).
#[tauri::command]
#[specta::specta]
pub fn check_screen_recording_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        use screencapturekit::prelude::*;
        SCShareableContent::get().is_ok()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

#[tauri::command]
#[specta::specta]
pub async fn start_meeting(
    _app: AppHandle,
    meeting_manager: State<'_, Arc<MeetingManager>>,
) -> Result<String, String> {
    meeting_manager
        .start_meeting()
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn stop_meeting(
    _app: AppHandle,
    meeting_manager: State<'_, Arc<MeetingManager>>,
    session_id: String,
) -> Result<(), String> {
    meeting_manager
        .stop_meeting(&session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_meeting(
    _app: AppHandle,
    meeting_manager: State<'_, Arc<MeetingManager>>,
    session_id: String,
) -> Result<(), String> {
    meeting_manager
        .cancel_meeting(&session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_meetings(
    _app: AppHandle,
    meeting_manager: State<'_, Arc<MeetingManager>>,
) -> Result<Vec<MeetingSummary>, String> {
    meeting_manager
        .get_meetings()
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_meeting_transcript(
    _app: AppHandle,
    meeting_manager: State<'_, Arc<MeetingManager>>,
    session_id: String,
) -> Result<Vec<MeetingSegment>, String> {
    meeting_manager
        .get_meeting_transcript(&session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_meeting(
    _app: AppHandle,
    meeting_manager: State<'_, Arc<MeetingManager>>,
    session_id: String,
) -> Result<(), String> {
    meeting_manager
        .delete_meeting(&session_id)
        .map_err(|e| e.to_string())
}
