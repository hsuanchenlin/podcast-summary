use std::path::Path;

use anyhow::{Context, Result};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, Decoder};
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

const WHISPER_SAMPLE_RATE: u32 = 16_000;

fn stereo_to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    samples
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Linear interpolation resampler — good enough for speech/transcription.
fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || samples.is_empty() {
        return samples.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = (samples.len() as f64 / ratio).ceil() as usize;
    let mut output = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let src_pos = i as f64 * ratio;
        let idx = src_pos as usize;
        let frac = (src_pos - idx as f64) as f32;

        let sample = if idx + 1 < samples.len() {
            samples[idx] * (1.0 - frac) + samples[idx + 1] * frac
        } else if idx < samples.len() {
            samples[idx]
        } else {
            0.0
        };
        output.push(sample);
    }

    output
}

/// Streaming audio decoder that yields chunks of 16kHz mono f32 samples.
/// This avoids loading the entire audio file into memory at once.
pub struct ChunkedAudioDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    sample_buf: Option<SampleBuffer<f32>>,
    track_id: u32,
    source_rate: u32,
    channels: usize,
    total_duration_secs: Option<f64>,
    finished: bool,
}

impl ChunkedAudioDecoder {
    pub fn open(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)
            .with_context(|| format!("Failed to open audio file: {}", path.display()))?;
        let source = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                source,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .with_context(|| format!("Failed to probe audio format: {}", path.display()))?;

        let format = probed.format;

        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| anyhow::anyhow!("No audio track found in {}", path.display()))?;

        let track_id = track.id;
        let source_rate = track.codec_params.sample_rate.unwrap_or(44100);
        let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(1);

        let total_duration_secs = track
            .codec_params
            .n_frames
            .zip(track.codec_params.time_base)
            .map(|(n_frames, time_base)| {
                let t = time_base.calc_time(n_frames);
                t.seconds as f64 + t.frac
            });

        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &Default::default())
            .with_context(|| "Failed to create audio decoder")?;

        Ok(Self {
            format,
            decoder,
            sample_buf: None,
            track_id,
            source_rate,
            channels,
            total_duration_secs,
            finished: false,
        })
    }

    pub fn total_duration_secs(&self) -> Option<f64> {
        self.total_duration_secs
    }

    /// Decode up to `max_seconds` of audio, returning 16kHz mono f32 samples.
    /// Returns `None` when the audio is exhausted.
    pub fn next_chunk(&mut self, max_seconds: u32) -> Result<Option<Vec<f32>>> {
        if self.finished {
            return Ok(None);
        }

        let max_source_samples = (max_seconds as usize)
            .saturating_mul(self.source_rate as usize)
            .saturating_mul(self.channels);
        let mut chunk_samples: Vec<f32> = Vec::new();

        while chunk_samples.len() < max_source_samples {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(symphonia::core::errors::Error::ResetRequired) => continue,
                Err(_) => {
                    self.finished = true;
                    break;
                }
            };

            if packet.track_id() != self.track_id {
                continue;
            }

            let audio_buf = match self.decoder.decode(&packet) {
                Ok(buf) => buf,
                Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
                Err(_) => {
                    self.finished = true;
                    break;
                }
            };

            let buf = self.sample_buf.get_or_insert_with(|| {
                SampleBuffer::<f32>::new(audio_buf.capacity() as u64, *audio_buf.spec())
            });
            buf.copy_interleaved_ref(audio_buf);
            chunk_samples.extend_from_slice(buf.samples());
        }

        if chunk_samples.is_empty() {
            return Ok(None);
        }

        let mono = if self.channels > 1 {
            stereo_to_mono(&chunk_samples, self.channels)
        } else {
            chunk_samples
        };

        if self.source_rate != WHISPER_SAMPLE_RATE {
            Ok(Some(resample(&mono, self.source_rate, WHISPER_SAMPLE_RATE)))
        } else {
            Ok(Some(mono))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_to_mono_basic() {
        // Stereo: [L, R, L, R] -> mono: [avg, avg]
        let stereo = vec![1.0, 0.0, 0.5, 0.5];
        let mono = stereo_to_mono(&stereo, 2);
        assert_eq!(mono.len(), 2);
        assert!((mono[0] - 0.5).abs() < f32::EPSILON);
        assert!((mono[1] - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn stereo_to_mono_silence() {
        let stereo = vec![0.0, 0.0, 0.0, 0.0];
        let mono = stereo_to_mono(&stereo, 2);
        assert_eq!(mono, vec![0.0, 0.0]);
    }

    #[test]
    fn stereo_to_mono_multichannel() {
        // 4-channel: each frame is 4 samples
        let samples = vec![1.0, 2.0, 3.0, 4.0]; // one frame
        let mono = stereo_to_mono(&samples, 4);
        assert_eq!(mono.len(), 1);
        assert!((mono[0] - 2.5).abs() < f32::EPSILON);
    }

    #[test]
    fn resample_same_rate() {
        let samples = vec![1.0, 2.0, 3.0];
        let result = resample(&samples, 44100, 44100);
        assert_eq!(result, samples);
    }

    #[test]
    fn resample_empty() {
        let result = resample(&[], 44100, 16000);
        assert!(result.is_empty());
    }

    #[test]
    fn resample_downsample_length() {
        // 44100 -> 16000: output should be shorter
        let samples: Vec<f32> = (0..44100).map(|i| i as f32 / 44100.0).collect();
        let result = resample(&samples, 44100, 16000);
        // Should be approximately 16000 samples
        let expected_len = (44100.0_f64 * 16000.0 / 44100.0).ceil() as usize;
        assert_eq!(result.len(), expected_len);
    }

    #[test]
    fn resample_upsample_interpolation() {
        // Simple case: 2 samples at rate 1 -> rate 2 should interpolate
        let samples = vec![0.0, 1.0];
        let result = resample(&samples, 1, 2);
        // Should have ~4 samples with interpolated values
        assert!(result.len() >= 3);
        // First sample should be 0.0
        assert!((result[0] - 0.0).abs() < f32::EPSILON);
    }
}
