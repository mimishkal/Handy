//! System audio capture via ScreenCaptureKit (macOS 12.3+).
//!
//! Captures audio played through speakers/headphones (e.g., Zoom/Meet voices)
//! and accumulates samples for periodic draining by the MeetingManager.

#[cfg(target_os = "macos")]
mod inner {
    use screencapturekit::prelude::*;
    use std::sync::{Arc, Mutex};

    use crate::audio_toolkit::constants;

    /// Overlap kept after drain (2 seconds at 16 kHz) to avoid cutting words.
    const DRAIN_OVERLAP_SAMPLES: usize = 32_000;

    /// Shared buffer for accumulating audio samples from the ScreenCaptureKit callback.
    struct SharedBuffer {
        samples: Vec<f32>,
    }

    /// Captures macOS system audio using ScreenCaptureKit.
    ///
    /// Audio is accumulated internally; call `drain()` to extract samples.
    pub struct SystemAudioRecorder {
        stream: Option<SCStream>,
        buffer: Arc<Mutex<SharedBuffer>>,
    }

    impl SystemAudioRecorder {
        pub fn new() -> Self {
            Self {
                stream: None,
                buffer: Arc::new(Mutex::new(SharedBuffer {
                    samples: Vec::new(),
                })),
            }
        }

        /// Start capturing system audio.
        ///
        /// Requires Screen Recording permission on macOS.
        /// Configures audio at 16 kHz mono (Whisper-compatible).
        pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            if self.stream.is_some() {
                return Ok(()); // already running
            }

            // We need a display for the content filter, even for audio-only capture
            let content = SCShareableContent::get()
                .map_err(|e| format!("Failed to get shareable content (Screen Recording permission may be needed): {e}"))?;

            let display = content
                .displays()
                .into_iter()
                .next()
                .ok_or("No display found for ScreenCaptureKit")?;

            let filter = SCContentFilter::create()
                .with_display(&display)
                .with_excluding_windows(&[])
                .build();

            // Audio-only config: 16 kHz mono, minimal video (required by API)
            let config = SCStreamConfiguration::new()
                .with_captures_audio(true)
                .with_excludes_current_process_audio(true)
                .with_sample_rate(constants::WHISPER_SAMPLE_RATE as i32)
                .with_channel_count(1)
                .with_width(2)
                .with_height(2);

            let buffer = self.buffer.clone();

            let mut stream = SCStream::new(&filter, &config);

            stream.add_output_handler(
                move |sample: CMSampleBuffer, of_type: SCStreamOutputType| {
                    if matches!(of_type, SCStreamOutputType::Audio) {
                        if let Some(audio_buffer_list) = sample.audio_buffer_list() {
                            for audio_buf in audio_buffer_list.iter() {
                                let raw_bytes = audio_buf.data();
                                // ScreenCaptureKit outputs 32-bit float PCM
                                let float_samples: &[f32] = unsafe {
                                    std::slice::from_raw_parts(
                                        raw_bytes.as_ptr() as *const f32,
                                        raw_bytes.len() / std::mem::size_of::<f32>(),
                                    )
                                };
                                if let Ok(mut buf) = buffer.lock() {
                                    buf.samples.extend_from_slice(float_samples);
                                }
                            }
                        }
                    }
                },
                SCStreamOutputType::Audio,
            );

            stream
                .start_capture()
                .map_err(|e| format!("Failed to start system audio capture: {e}"))?;

            log::info!("System audio capture started via ScreenCaptureKit");
            self.stream = Some(stream);
            Ok(())
        }

        /// Extract accumulated samples without stopping capture.
        /// Keeps a 2-second overlap tail in the buffer for word continuity.
        pub fn drain(&self) -> Vec<f32> {
            let mut buf = self.buffer.lock().unwrap();
            let len = buf.samples.len();
            if len > DRAIN_OVERLAP_SAMPLES {
                let drained: Vec<f32> = buf.samples.drain(..len - DRAIN_OVERLAP_SAMPLES).collect();
                drained
            } else if !buf.samples.is_empty() {
                buf.samples.clone() // not enough yet, return clone
            } else {
                Vec::new()
            }
        }

        /// Stop capture and return all remaining samples.
        pub fn stop(&mut self) -> Vec<f32> {
            if let Some(stream) = self.stream.take() {
                if let Err(e) = stream.stop_capture() {
                    log::error!("Failed to stop system audio capture: {e}");
                }
                log::info!("System audio capture stopped");
            }
            let mut buf = self.buffer.lock().unwrap();
            std::mem::take(&mut buf.samples)
        }

        /// Check if capture is currently running.
        pub fn is_running(&self) -> bool {
            self.stream.is_some()
        }
    }

    impl Drop for SystemAudioRecorder {
        fn drop(&mut self) {
            if self.stream.is_some() {
                let _ = self.stop();
            }
        }
    }
}

/// Stub for non-macOS platforms.
#[cfg(not(target_os = "macos"))]
mod inner {
    pub struct SystemAudioRecorder;

    impl SystemAudioRecorder {
        pub fn new() -> Self {
            Self
        }

        pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            Err("System audio capture is only supported on macOS".into())
        }

        pub fn drain(&self) -> Vec<f32> {
            Vec::new()
        }

        pub fn stop(&mut self) -> Vec<f32> {
            Vec::new()
        }

        pub fn is_running(&self) -> bool {
            false
        }
    }
}

pub use inner::SystemAudioRecorder;
