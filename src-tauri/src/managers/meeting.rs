//! Meeting mode manager — orchestrates dual-stream recording (mic + system audio)
//! with periodic incremental transcription.

use crate::audio_toolkit::{save_wav_file, SystemAudioRecorder};
use crate::managers::audio::AudioRecordingManager;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::get_settings;
use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use log::{debug, error, info, warn};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;

/* ──────────────────────────── types ──────────────────────────── */

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub enum MeetingStatus {
    Recording,
    Completed,
    Cancelled,
}

impl MeetingStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Recording => "recording",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "completed" => Self::Completed,
            "cancelled" => Self::Cancelled,
            _ => Self::Recording,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct MeetingSummary {
    pub id: String,
    pub title: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub duration_seconds: Option<i64>,
    pub status: String,
    pub total_segments: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct MeetingSegment {
    pub id: i64,
    pub meeting_id: String,
    pub segment_index: i64,
    pub started_at: i64,
    pub duration_seconds: f64,
    pub transcription_text: String,
    pub audio_file_name: String,
    pub speaker: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct MeetingSegmentEvent {
    pub session_id: String,
    pub segment_index: i64,
    pub text: String,
    pub speaker: String,
    pub started_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct MeetingStateEvent {
    pub session_id: String,
    pub status: String,
}

/* ──────────────────────────── manager ──────────────────────────── */

struct ActiveMeeting {
    session_id: String,
    started_at: i64,
    segment_index: i64,
    stop_tx: Option<oneshot::Sender<()>>,
}

pub struct MeetingManager {
    app_handle: AppHandle,
    meetings_dir: PathBuf,
    db_path: PathBuf,
    active: Arc<Mutex<Option<ActiveMeeting>>>,
    system_audio: Arc<Mutex<SystemAudioRecorder>>,
}

impl MeetingManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let app_data_dir = crate::portable::app_data_dir(app_handle)?;
        let meetings_dir = app_data_dir.join("meetings");
        let db_path = app_data_dir.join("history.db");

        if !meetings_dir.exists() {
            std::fs::create_dir_all(&meetings_dir)?;
        }

        Ok(Self {
            app_handle: app_handle.clone(),
            meetings_dir,
            db_path,
            active: Arc::new(Mutex::new(None)),
            system_audio: Arc::new(Mutex::new(SystemAudioRecorder::new())),
        })
    }

    fn get_connection(&self) -> Result<Connection> {
        Ok(Connection::open(&self.db_path)?)
    }

    /// Check if a meeting is currently active.
    pub fn is_active(&self) -> bool {
        self.active.lock().unwrap().is_some()
    }

    /// Get the active meeting session ID (if any).
    pub fn active_session_id(&self) -> Option<String> {
        self.active
            .lock()
            .unwrap()
            .as_ref()
            .map(|m| m.session_id.clone())
    }

    /* ──────────────── start meeting ──────────────── */

    pub fn start_meeting(&self) -> Result<String> {
        // Check mutual exclusion with dictation
        let recording_manager = self.app_handle.state::<Arc<AudioRecordingManager>>();
        if recording_manager.is_recording() {
            anyhow::bail!("Cannot start meeting while dictation is recording");
        }

        let mut active = self.active.lock().unwrap();
        if active.is_some() {
            anyhow::bail!("A meeting is already in progress");
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let started_at = Utc::now().timestamp();
        let title = format_meeting_title(started_at);

        // Create session directory
        let session_dir = self.meetings_dir.join(&session_id);
        std::fs::create_dir_all(&session_dir)?;

        // Insert meeting record
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO meetings (id, title, started_at, status) VALUES (?1, ?2, ?3, ?4)",
            params![session_id, title, started_at, "recording"],
        )?;

        // Start microphone recording
        if let Err(e) = recording_manager.try_start_recording("meeting") {
            // Clean up DB record
            let _ = conn.execute("DELETE FROM meetings WHERE id = ?1", params![session_id]);
            anyhow::bail!("Failed to start microphone: {e}");
        }

        // Start system audio capture (best-effort, continue without it)
        {
            let mut sys_audio = self.system_audio.lock().unwrap();
            match sys_audio.start() {
                Ok(()) => info!("System audio capture started successfully"),
                Err(e) => warn!("System audio capture unavailable: {e}. Meeting will record mic only."),
            }
        }

        // Create stop channel
        let (stop_tx, stop_rx) = oneshot::channel::<()>();

        *active = Some(ActiveMeeting {
            session_id: session_id.clone(),
            started_at,
            segment_index: 0,
            stop_tx: Some(stop_tx),
        });

        // Emit state event
        let _ = self.app_handle.emit(
            "meeting-state-changed",
            MeetingStateEvent {
                session_id: session_id.clone(),
                status: "recording".to_string(),
            },
        );

        // Spawn the periodic drain-and-transcribe loop
        let app_handle = self.app_handle.clone();
        let active_arc = self.active.clone();
        let system_audio = self.system_audio.clone();
        let meetings_dir = self.meetings_dir.clone();
        let db_path = self.db_path.clone();
        let sid = session_id.clone();

        tauri::async_runtime::spawn(async move {
            meeting_loop(
                app_handle,
                active_arc,
                system_audio,
                meetings_dir,
                db_path,
                sid,
                stop_rx,
            )
            .await;
        });

        info!("Meeting started: {session_id}");
        Ok(session_id)
    }

    /* ──────────────── stop meeting ──────────────── */

    pub fn stop_meeting(&self, session_id: &str) -> Result<()> {
        let mut active = self.active.lock().unwrap();
        let meeting = active.as_mut().ok_or_else(|| anyhow::anyhow!("No active meeting"))?;

        if meeting.session_id != session_id {
            anyhow::bail!("Session ID mismatch");
        }

        // Signal the loop to stop
        if let Some(tx) = meeting.stop_tx.take() {
            let _ = tx.send(());
        }

        // The loop handles final drain, transcription, and DB update.
        // We just clear the active state.
        *active = None;

        info!("Meeting stop requested: {session_id}");
        Ok(())
    }

    /* ──────────────── cancel meeting ──────────────── */

    pub fn cancel_meeting(&self, session_id: &str) -> Result<()> {
        let mut active = self.active.lock().unwrap();
        let meeting = active.as_mut().ok_or_else(|| anyhow::anyhow!("No active meeting"))?;

        if meeting.session_id != session_id {
            anyhow::bail!("Session ID mismatch");
        }

        // Signal the loop to stop
        if let Some(tx) = meeting.stop_tx.take() {
            let _ = tx.send(());
        }
        *active = None;

        // Stop recordings
        let recording_manager = self.app_handle.state::<Arc<AudioRecordingManager>>();
        recording_manager.cancel_recording();
        self.system_audio.lock().unwrap().stop();

        // Update DB status
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE meetings SET status = 'cancelled' WHERE id = ?1",
            params![session_id],
        )?;

        let _ = self.app_handle.emit(
            "meeting-state-changed",
            MeetingStateEvent {
                session_id: session_id.to_string(),
                status: "cancelled".to_string(),
            },
        );

        info!("Meeting cancelled: {session_id}");
        Ok(())
    }

    /* ──────────────── queries ──────────────── */

    pub fn get_meetings(&self) -> Result<Vec<MeetingSummary>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, started_at, ended_at, duration_seconds, status, total_segments
             FROM meetings ORDER BY started_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(MeetingSummary {
                id: row.get("id")?,
                title: row.get("title")?,
                started_at: row.get("started_at")?,
                ended_at: row.get("ended_at")?,
                duration_seconds: row.get("duration_seconds")?,
                status: row.get("status")?,
                total_segments: row.get("total_segments")?,
            })
        })?;
        let mut meetings = Vec::new();
        for row in rows {
            meetings.push(row?);
        }
        Ok(meetings)
    }

    pub fn get_meeting_transcript(&self, session_id: &str) -> Result<Vec<MeetingSegment>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, meeting_id, segment_index, started_at, duration_seconds,
                    transcription_text, audio_file_name, speaker
             FROM meeting_segments WHERE meeting_id = ?1 ORDER BY segment_index",
        )?;
        let rows = stmt.query_map(params![session_id], |row| {
            Ok(MeetingSegment {
                id: row.get("id")?,
                meeting_id: row.get("meeting_id")?,
                segment_index: row.get("segment_index")?,
                started_at: row.get("started_at")?,
                duration_seconds: row.get("duration_seconds")?,
                transcription_text: row.get("transcription_text")?,
                audio_file_name: row.get("audio_file_name")?,
                speaker: row.get("speaker")?,
            })
        })?;
        let mut segments = Vec::new();
        for row in rows {
            segments.push(row?);
        }
        Ok(segments)
    }

    pub fn delete_meeting(&self, session_id: &str) -> Result<()> {
        // Delete audio files
        let session_dir = self.meetings_dir.join(session_id);
        if session_dir.exists() {
            std::fs::remove_dir_all(&session_dir)?;
        }
        // Delete from DB (CASCADE will delete segments)
        let conn = self.get_connection()?;
        conn.execute("DELETE FROM meetings WHERE id = ?1", params![session_id])?;
        info!("Meeting deleted: {session_id}");
        Ok(())
    }
}

/* ──────────────── meeting loop (async) ──────────────── */

async fn meeting_loop(
    app_handle: AppHandle,
    active: Arc<Mutex<Option<ActiveMeeting>>>,
    system_audio: Arc<Mutex<SystemAudioRecorder>>,
    meetings_dir: PathBuf,
    db_path: PathBuf,
    session_id: String,
    mut stop_rx: oneshot::Receiver<()>,
) {
    let settings = get_settings(&app_handle);
    let chunk_interval = std::time::Duration::from_secs(settings.meeting_chunk_seconds as u64);
    let session_dir = meetings_dir.join(&session_id);

    // Ensure the transcription model is loaded before processing chunks
    let transcription_manager = app_handle.state::<Arc<TranscriptionManager>>();
    if !transcription_manager.is_model_loaded() {
        info!("Meeting: loading transcription model...");
        transcription_manager.initiate_model_load();
    }

    loop {
        // Wait for chunk interval or stop signal
        tokio::select! {
            _ = tokio::time::sleep(chunk_interval) => {},
            _ = &mut stop_rx => {
                // Finalize
                finalize_meeting(
                    &app_handle, &active, &system_audio, &session_dir, &db_path, &session_id,
                ).await;
                return;
            }
        }

        // Check if still active
        let is_active = active.lock().unwrap().is_some();
        if !is_active {
            return;
        }

        // Drain and transcribe mic audio
        let recording_manager = app_handle.state::<Arc<AudioRecordingManager>>();
        if let Some(mic_samples) = recording_manager.drain_samples() {
            process_chunk(
                &app_handle,
                &active,
                &session_dir,
                &db_path,
                &session_id,
                mic_samples,
                "me",
                "mic",
            )
            .await;
        }

        // Drain and transcribe system audio
        let sys_samples = system_audio.lock().unwrap().drain();
        if !sys_samples.is_empty() {
            process_chunk(
                &app_handle,
                &active,
                &session_dir,
                &db_path,
                &session_id,
                sys_samples,
                "others",
                "sys",
            )
            .await;
        }
    }
}

async fn process_chunk(
    app_handle: &AppHandle,
    active: &Arc<Mutex<Option<ActiveMeeting>>>,
    session_dir: &PathBuf,
    db_path: &PathBuf,
    session_id: &str,
    samples: Vec<f32>,
    speaker: &str,
    prefix: &str,
) {
    let segment_index = {
        let mut guard = active.lock().unwrap();
        if let Some(meeting) = guard.as_mut() {
            let idx = meeting.segment_index;
            meeting.segment_index += 1;
            idx
        } else {
            return;
        }
    };

    let started_at = Utc::now().timestamp();
    let sample_count = samples.len();
    let duration_seconds = sample_count as f64 / 16000.0;

    // Save chunk to disk
    let chunk_filename = format!("{prefix}_chunk_{segment_index:04}.wav");
    let chunk_path = session_dir.join(&chunk_filename);
    if let Err(e) = save_wav_file(&chunk_path, &samples).await {
        error!("Failed to save meeting chunk: {e}");
        return;
    }

    // Transcribe
    let transcription_manager = app_handle.state::<Arc<TranscriptionManager>>();
    let text = match transcription_manager.transcribe(samples) {
        Ok(t) => t,
        Err(e) => {
            error!("Transcription failed for meeting chunk: {e}");
            return;
        }
    };

    if text.trim().is_empty() {
        debug!("Empty transcription for {prefix} chunk {segment_index}, skipping");
        return;
    }

    // Save to DB
    if let Ok(conn) = Connection::open(db_path) {
        let _ = conn.execute(
            "INSERT INTO meeting_segments (meeting_id, segment_index, started_at, duration_seconds, transcription_text, audio_file_name, speaker)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![session_id, segment_index, started_at, duration_seconds, text, chunk_filename, speaker],
        );
        let _ = conn.execute(
            "UPDATE meetings SET total_segments = total_segments + 1 WHERE id = ?1",
            params![session_id],
        );
    }

    // Emit to frontend
    let _ = app_handle.emit(
        "meeting-segment",
        MeetingSegmentEvent {
            session_id: session_id.to_string(),
            segment_index,
            text: text.clone(),
            speaker: speaker.to_string(),
            started_at,
        },
    );

    debug!(
        "Meeting segment {segment_index} ({speaker}): {:.0}s, {} chars",
        duration_seconds,
        text.len()
    );
}

async fn finalize_meeting(
    app_handle: &AppHandle,
    active: &Arc<Mutex<Option<ActiveMeeting>>>,
    system_audio: &Arc<Mutex<SystemAudioRecorder>>,
    session_dir: &PathBuf,
    db_path: &PathBuf,
    session_id: &str,
) {
    info!("Finalizing meeting: {session_id}");

    // Stop microphone and get final samples
    let recording_manager = app_handle.state::<Arc<AudioRecordingManager>>();
    if let Some(final_mic) = recording_manager.stop_recording("meeting") {
        if !final_mic.is_empty() {
            process_chunk(
                app_handle, active, session_dir, db_path, session_id, final_mic, "me", "mic",
            )
            .await;
        }
    }

    // Stop system audio and get final samples
    let final_sys = system_audio.lock().unwrap().stop();
    if !final_sys.is_empty() {
        process_chunk(
            app_handle, active, session_dir, db_path, session_id, final_sys, "others", "sys",
        )
        .await;
    }

    // Update DB
    let ended_at = Utc::now().timestamp();
    if let Ok(conn) = Connection::open(db_path) {
        let started_at: i64 = conn
            .query_row(
                "SELECT started_at FROM meetings WHERE id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .unwrap_or(ended_at);
        let duration = ended_at - started_at;
        let _ = conn.execute(
            "UPDATE meetings SET ended_at = ?1, duration_seconds = ?2, status = 'completed' WHERE id = ?3",
            params![ended_at, duration, session_id],
        );
    }

    // Emit completed
    let _ = app_handle.emit(
        "meeting-state-changed",
        MeetingStateEvent {
            session_id: session_id.to_string(),
            status: "completed".to_string(),
        },
    );

    info!("Meeting finalized: {session_id}");
}

/* ──────────────── helpers ──────────────── */

fn format_meeting_title(timestamp: i64) -> String {
    if let Some(utc) = DateTime::from_timestamp(timestamp, 0) {
        let local = utc.with_timezone(&Local);
        format!("Meeting - {}", local.format("%B %e, %Y - %l:%M%p"))
    } else {
        format!("Meeting {timestamp}")
    }
}
