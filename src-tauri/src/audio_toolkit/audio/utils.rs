use anyhow::{Context, Result};
use hound::{WavSpec, WavWriter};
use log::debug;
use rubato::{FftFixedIn, Resampler};
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

const TARGET_SAMPLE_RATE: u32 = 16000;

/// Read an audio file (WAV, MP3, M4A, FLAC, OGG) and return 16kHz mono f32 samples.
pub fn read_audio_file<P: AsRef<Path>>(file_path: P) -> Result<Vec<f32>> {
    let path = file_path.as_ref();
    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open audio file: {:?}", path))?;

    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // Provide a hint based on file extension
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .context("Failed to probe audio format")?;

    let mut format = probed.format;
    let track = format
        .default_track()
        .context("No audio track found")?
        .clone();

    let channels = track.codec_params.channels.map_or(1, |c| c.count());
    let sample_rate = track
        .codec_params
        .sample_rate
        .context("Unknown sample rate")?;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("Failed to create audio decoder")?;

    // Decode all packets to f32 samples
    let mut raw_samples: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(e.into()),
        };

        if packet.track_id() != track.id {
            continue;
        }

        let decoded = decoder.decode(&packet)?;
        let spec = *decoded.spec();
        let num_frames = decoded.capacity();

        let mut sample_buf = SampleBuffer::<f32>::new(num_frames as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);
        raw_samples.extend_from_slice(sample_buf.samples());
    }

    // Convert to mono if multi-channel
    let mono_samples = if channels > 1 {
        raw_samples
            .chunks_exact(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        raw_samples
    };

    // Resample to 16kHz if needed
    if sample_rate == TARGET_SAMPLE_RATE {
        debug!(
            "Read audio file {:?}: {} samples, {}Hz mono, no resampling needed",
            path,
            mono_samples.len(),
            sample_rate
        );
        Ok(mono_samples)
    } else {
        let resampled = resample_batch(&mono_samples, sample_rate as usize, TARGET_SAMPLE_RATE as usize)?;
        debug!(
            "Read audio file {:?}: {} -> {} samples, {}Hz -> {}Hz",
            path,
            mono_samples.len(),
            resampled.len(),
            sample_rate,
            TARGET_SAMPLE_RATE
        );
        Ok(resampled)
    }
}

/// Resample a batch of mono audio samples from one sample rate to another.
fn resample_batch(samples: &[f32], from_hz: usize, to_hz: usize) -> Result<Vec<f32>> {
    let chunk_size = 1024;
    let mut resampler = FftFixedIn::<f32>::new(from_hz, to_hz, chunk_size, 1, 1)
        .context("Failed to create resampler")?;

    let mut output = Vec::new();

    // Process full chunks
    for chunk in samples.chunks(chunk_size) {
        let input = if chunk.len() < chunk_size {
            // Pad the last chunk with zeros
            let mut padded = chunk.to_vec();
            padded.resize(chunk_size, 0.0);
            padded
        } else {
            chunk.to_vec()
        };

        let result = resampler.process(&[&input], None)?;
        output.extend_from_slice(&result[0]);
    }

    // Trim output to expected length based on ratio
    let expected_len = (samples.len() as f64 * to_hz as f64 / from_hz as f64).round() as usize;
    output.truncate(expected_len);

    Ok(output)
}

/// Save audio samples as a WAV file
pub async fn save_wav_file<P: AsRef<Path>>(file_path: P, samples: &[f32]) -> Result<()> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = WavWriter::create(file_path.as_ref(), spec)?;

    // Convert f32 samples to i16 for WAV
    for sample in samples {
        let sample_i16 = (sample * i16::MAX as f32) as i16;
        writer.write_sample(sample_i16)?;
    }

    writer.finalize()?;
    debug!("Saved WAV file: {:?}", file_path.as_ref());
    Ok(())
}
