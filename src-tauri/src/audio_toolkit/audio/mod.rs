// Re-export all audio components
mod device;
mod recorder;
mod resampler;
mod system_audio;
mod utils;
mod visualizer;

pub use device::{list_input_devices, list_output_devices, CpalDeviceInfo};
pub use recorder::AudioRecorder;
pub use resampler::FrameResampler;
pub use system_audio::SystemAudioRecorder;
pub use utils::{read_audio_file, save_wav_file};
pub use visualizer::AudioVisualiser;
