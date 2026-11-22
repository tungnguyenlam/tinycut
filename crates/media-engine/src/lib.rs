use serde::Deserialize;
use std::path::Path;
use std::process::Command;
use tinycut_core_domain::{DomainError, DomainResult, MediaRef};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AudioProxySpec {
    pub sample_rate_hz: u32,
    pub bitrate_kbps: u32,
    pub channels: u8,
}

impl Default for AudioProxySpec {
    fn default() -> Self {
        Self {
            sample_rate_hz: 16_000,
            bitrate_kbps: 48,
            channels: 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VideoProxySpec {
    pub max_height: u32,
    pub target_fps: f32,
}

impl Default for VideoProxySpec {
    fn default() -> Self {
        Self {
            max_height: 720,
            target_fps: 12.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecodedFrame {
    pub frame_index: u64,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Smart Proxy Offloading & local media extraction interface.
/// Native implementations will bind AVFoundation (Apple), MediaExtractor (Android),
/// and symphonia / LGPL FFmpeg (Desktop).
pub trait MediaExtractor: Send + Sync {
    fn probe_media(&self, path: &Path) -> DomainResult<MediaRef>;
    fn extract_pcm_16k(&self, media: &MediaRef) -> DomainResult<Vec<f32>>;
    fn build_opus_proxy(&self, media: &MediaRef, spec: &AudioProxySpec) -> DomainResult<Vec<u8>>;
    fn build_vision_proxy(&self, media: &MediaRef, spec: &VideoProxySpec) -> DomainResult<Vec<u8>>;
    fn slice_audio_clip(
        &self,
        media: &MediaRef,
        start_ms: u64,
        end_ms: u64,
    ) -> DomainResult<Vec<u8>>;
}

/// Desktop LGPL FFmpeg / ffprobe media extractor.
#[derive(Debug, Clone)]
pub struct FfmpegMediaExtractor {
    pub ffmpeg_bin: String,
    pub ffprobe_bin: String,
}

impl Default for FfmpegMediaExtractor {
    fn default() -> Self {
        Self {
            ffmpeg_bin: "ffmpeg".into(),
            ffprobe_bin: "ffprobe".into(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    #[serde(default)]
    streams: Vec<FfprobeStream>,
    format: Option<FfprobeFormat>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    codec_type: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    r_frame_rate: Option<String>,
    avg_frame_rate: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FfprobeFormat {
    duration: Option<String>,
}

fn parse_fps(rate: Option<&str>) -> Option<f32> {
    let raw = rate?;
    if let Some((num, den)) = raw.split_once('/') {
        let n: f32 = num.parse().ok()?;
        let d: f32 = den.parse().ok()?;
        if d > 0.0 && n > 0.0 {
            return Some(n / d);
        }
        None
    } else {
        let val: f32 = raw.parse().ok()?;
        (val > 0.0).then_some(val)
    }
}

impl FfmpegMediaExtractor {
    pub fn new(ffmpeg_bin: impl Into<String>, ffprobe_bin: impl Into<String>) -> Self {
        Self {
            ffmpeg_bin: ffmpeg_bin.into(),
            ffprobe_bin: ffprobe_bin.into(),
        }
    }

    fn run_ffmpeg_bytes(&self, args: &[&str]) -> DomainResult<Vec<u8>> {
        let output = Command::new(&self.ffmpeg_bin)
            .args(args)
            .output()
            .map_err(|e| DomainError::Media(format!("failed to spawn {}: {e}", self.ffmpeg_bin)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DomainError::Media(format!(
                "ffmpeg exited with {}: {}",
                output.status,
                stderr.trim()
            )));
        }
        Ok(output.stdout)
    }

    /// Extracts sampled RGBA video frames for Vision Privacy Shield keyframe analysis.
    pub fn extract_sampled_frames_rgba(
        &self,
        media: &MediaRef,
        sample_fps: f32,
        max_width: u32,
    ) -> DomainResult<Vec<DecodedFrame>> {
        if media.width == 0 || media.height == 0 {
            return Err(DomainError::Media("media has no video dimensions".into()));
        }
        let target_w = media.width.min(max_width).max(2) & !1;
        let target_h = (((media.height as f64) * (target_w as f64) / (media.width as f64)).round()
            as u32)
            .max(2)
            & !1;
        let vf = format!("fps={sample_fps:.3},scale={target_w}:{target_h}");
        let raw = self.run_ffmpeg_bytes(&[
            "-v",
            "error",
            "-i",
            &media.path,
            "-vf",
            &vf,
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "pipe:1",
        ])?;

        let frame_bytes = (target_w as usize) * (target_h as usize) * 4;
        if frame_bytes == 0 {
            return Ok(Vec::new());
        }
        let step = if sample_fps > 0.0 && media.fps > 0.0 {
            (media.fps / sample_fps).max(1.0)
        } else {
            1.0
        };

        let frames = raw
            .chunks_exact(frame_bytes)
            .enumerate()
            .map(|(i, chunk)| DecodedFrame {
                frame_index: ((i as f32) * step).round() as u64,
                width: target_w,
                height: target_h,
                rgba: chunk.to_vec(),
            })
            .collect();
        Ok(frames)
    }
}

impl MediaExtractor for FfmpegMediaExtractor {
    fn probe_media(&self, path: &Path) -> DomainResult<MediaRef> {
        let path_str = path
            .to_str()
            .ok_or_else(|| DomainError::Media("non-UTF-8 media path".into()))?;
        let output = Command::new(&self.ffprobe_bin)
            .args([
                "-v",
                "error",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
                path_str,
            ])
            .output()
            .map_err(|e| {
                DomainError::Media(format!("failed to spawn {}: {e}", self.ffprobe_bin))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DomainError::Media(format!(
                "ffprobe failed: {}",
                stderr.trim()
            )));
        }

        let parsed: FfprobeOutput = serde_json::from_slice(&output.stdout)
            .map_err(|e| DomainError::Media(format!("invalid ffprobe json: {e}")))?;

        let duration_ms = parsed
            .format
            .and_then(|f| f.duration)
            .and_then(|d| d.parse::<f64>().ok())
            .map(|secs| (secs * 1000.0).round() as u64)
            .unwrap_or(0);

        let video = parsed
            .streams
            .iter()
            .find(|s| s.codec_type.as_deref() == Some("video"));

        let (width, height, fps) = match video {
            Some(v) => (
                v.width.unwrap_or(0),
                v.height.unwrap_or(0),
                parse_fps(v.avg_frame_rate.as_deref())
                    .or_else(|| parse_fps(v.r_frame_rate.as_deref()))
                    .unwrap_or(30.0),
            ),
            None => (0, 0, 0.0),
        };

        Ok(MediaRef {
            id: Uuid::new_v4(),
            path: path_str.to_string(),
            duration_ms,
            width,
            height,
            fps,
        })
    }

    fn extract_pcm_16k(&self, media: &MediaRef) -> DomainResult<Vec<f32>> {
        let bytes = self.run_ffmpeg_bytes(&[
            "-v",
            "error",
            "-i",
            &media.path,
            "-vn",
            "-ac",
            "1",
            "-ar",
            "16000",
            "-f",
            "f32le",
            "pipe:1",
        ])?;

        let samples = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| f32::from_le_bytes(*chunk))
            .collect();
        Ok(samples)
    }

    fn build_opus_proxy(&self, media: &MediaRef, spec: &AudioProxySpec) -> DomainResult<Vec<u8>> {
        let ar = spec.sample_rate_hz.to_string();
        let ac = spec.channels.to_string();
        let bitrate = format!("{}k", spec.bitrate_kbps);
        self.run_ffmpeg_bytes(&[
            "-v",
            "error",
            "-i",
            &media.path,
            "-vn",
            "-ac",
            &ac,
            "-ar",
            &ar,
            "-c:a",
            "libopus",
            "-b:a",
            &bitrate,
            "-f",
            "ogg",
            "pipe:1",
        ])
    }

    fn build_vision_proxy(&self, media: &MediaRef, spec: &VideoProxySpec) -> DomainResult<Vec<u8>> {
        let vf = format!("fps={:.3},scale=-2:min(ih\\,{})", spec.target_fps, spec.max_height);
        self.run_ffmpeg_bytes(&[
            "-v",
            "error",
            "-i",
            &media.path,
            "-an",
            "-vf",
            &vf,
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "28",
            "-movflags",
            "frag_keyframe+empty_moov",
            "-f",
            "mp4",
            "pipe:1",
        ])
    }

    fn slice_audio_clip(
        &self,
        media: &MediaRef,
        start_ms: u64,
        end_ms: u64,
    ) -> DomainResult<Vec<u8>> {
        if end_ms <= start_ms {
            return Err(DomainError::Media("end_ms must be greater than start_ms".into()));
        }
        let start_s = format!("{:.3}", start_ms as f64 / 1000.0);
        let dur_s = format!("{:.3}", (end_ms - start_ms) as f64 / 1000.0);
        self.run_ffmpeg_bytes(&[
            "-v",
            "error",
            "-ss",
            &start_s,
            "-t",
            &dur_s,
            "-i",
            &media.path,
            "-vn",
            "-ac",
            "1",
            "-ar",
            "16000",
            "-c:a",
            "libopus",
            "-b:a",
            "48k",
            "-f",
            "ogg",
            "pipe:1",
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_extract_pcm_and_slice_with_ffmpeg() {
        let tmp_wav = std::env::temp_dir().join(format!("tinycut_test_{}.wav", Uuid::new_v4()));
        let status = Command::new("ffmpeg")
            .args([
                "-y",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=1",
                tmp_wav.to_str().unwrap(),
            ])
            .status()
            .expect("ffmpeg must be installed to run media-engine tests");
        assert!(status.success());

        let extractor = FfmpegMediaExtractor::default();
        let media = extractor.probe_media(&tmp_wav).expect("probe should succeed");
        assert!(media.duration_ms >= 900 && media.duration_ms <= 1100);

        let pcm = extractor.extract_pcm_16k(&media).expect("pcm extraction should succeed");
        assert!(pcm.len() >= 15_000 && pcm.len() <= 17_000);

        let proxy = extractor
            .build_opus_proxy(&media, &AudioProxySpec::default())
            .expect("opus proxy should succeed");
        assert!(proxy.starts_with(b"OggS"));

        let clip = extractor
            .slice_audio_clip(&media, 100, 600)
            .expect("slice should succeed");
        assert!(clip.starts_with(b"OggS"));

        let _ = std::fs::remove_file(&tmp_wav);
    }
}
