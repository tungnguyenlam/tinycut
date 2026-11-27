use serde::Deserialize;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::UNIX_EPOCH;
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

/// Streaming RGBA frame iterator backed by a child `ffmpeg` process.
/// Reads one frame at a time from `ffmpeg`'s stdout pipe, keeping memory O(1)
/// with respect to video duration.
pub struct SampledFrameStream {
    child: Child,
    reader: BufReader<ChildStdout>,
    width: u32,
    height: u32,
    frame_bytes: usize,
    frame_step: f32,
    yielded_count: u64,
    finished: bool,
}

impl SampledFrameStream {
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

impl Iterator for SampledFrameStream {
    type Item = DomainResult<DecodedFrame>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished || self.frame_bytes == 0 {
            return None;
        }

        let mut buf = vec![0u8; self.frame_bytes];
        match self.reader.read_exact(&mut buf) {
            Ok(()) => {
                let frame_index = ((self.yielded_count as f32) * self.frame_step).round() as u64;
                self.yielded_count += 1;
                Some(Ok(DecodedFrame {
                    frame_index,
                    width: self.width,
                    height: self.height,
                    rgba: buf,
                }))
            }
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                self.finished = true;
                match self.child.wait() {
                    Ok(status) if status.success() => None,
                    Ok(status) => Some(Err(DomainError::Media(format!(
                        "ffmpeg frame stream exited with {status}"
                    )))),
                    Err(wait_err) => Some(Err(DomainError::Media(format!(
                        "failed waiting for ffmpeg frame stream: {wait_err}"
                    )))),
                }
            }
            Err(e) => {
                self.finished = true;
                let _ = self.child.kill();
                let _ = self.child.wait();
                Some(Err(DomainError::Media(format!(
                    "failed reading RGBA frame from ffmpeg: {e}"
                ))))
            }
        }
    }
}

impl Drop for SampledFrameStream {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.child.kill();
            let _ = self.child.wait();
            self.finished = true;
        }
    }
}

/// Streaming RGBA-to-MP4 video writer backed by a child `ffmpeg` process.
/// Accepts raw RGBA frames over stdin and optionally muxes audio copied from an existing source.
pub struct FrameVideoWriter {
    child: Option<Child>,
    stdin: Option<BufWriter<ChildStdin>>,
    expected_frame_bytes: usize,
}

impl FrameVideoWriter {
    pub fn new(
        ffmpeg_bin: &str,
        width: u32,
        height: u32,
        fps: f32,
        audio_source: Option<&Path>,
        output_path: &Path,
    ) -> DomainResult<Self> {
        if width == 0 || height == 0 {
            return Err(DomainError::Media(
                "video writer requires non-zero width and height".into(),
            ));
        }
        if !fps.is_finite() || fps <= 0.0 {
            return Err(DomainError::Media(
                "video writer requires positive finite fps".into(),
            ));
        }
        let output_str = output_path
            .to_str()
            .ok_or_else(|| DomainError::Media("non-UTF-8 output path".into()))?;
        if let Some(parent) = output_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|e| {
                DomainError::Media(format!(
                    "failed to create output directory {}: {e}",
                    parent.display()
                ))
            })?;
        }

        let size = format!("{width}x{height}");
        let fps_str = format!("{fps:.3}");
        let mut args: Vec<String> = vec![
            "-y".into(),
            "-v".into(),
            "error".into(),
            "-f".into(),
            "rawvideo".into(),
            "-pix_fmt".into(),
            "rgba".into(),
            "-s".into(),
            size,
            "-r".into(),
            fps_str,
            "-i".into(),
            "pipe:0".into(),
        ];

        if let Some(audio_path) = audio_source {
            let audio_str = audio_path
                .to_str()
                .ok_or_else(|| DomainError::Media("non-UTF-8 audio source path".into()))?;
            args.extend([
                "-i".into(),
                audio_str.into(),
                "-map".into(),
                "0:v:0".into(),
                "-map".into(),
                "1:a?".into(),
                "-c:a".into(),
                "copy".into(),
            ]);
        }

        args.extend([
            "-c:v".into(),
            "libx264".into(),
            "-preset".into(),
            "veryfast".into(),
            "-crf".into(),
            "18".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
            "-movflags".into(),
            "+faststart".into(),
            output_str.into(),
        ]);

        let mut child = Command::new(ffmpeg_bin)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| DomainError::Media(format!("failed to spawn {ffmpeg_bin}: {e}")))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| DomainError::Media("failed to open ffmpeg stdin pipe".into()))?;

        Ok(Self {
            child: Some(child),
            stdin: Some(BufWriter::new(stdin)),
            expected_frame_bytes: (width as usize) * (height as usize) * 4,
        })
    }

    /// Writes a single raw RGBA frame (`width * height * 4` bytes) to the encoder pipe.
    pub fn write_frame_rgba(&mut self, rgba: &[u8]) -> DomainResult<()> {
        if rgba.len() != self.expected_frame_bytes {
            return Err(DomainError::Media(format!(
                "frame byte length {} does not match expected {}",
                rgba.len(),
                self.expected_frame_bytes
            )));
        }
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| DomainError::Media("video writer is already finished".into()))?;
        stdin
            .write_all(rgba)
            .map_err(|e| DomainError::Media(format!("failed writing RGBA frame to ffmpeg: {e}")))
    }

    /// Flushes remaining frames, closes stdin, and waits for `ffmpeg` to finalize the output file.
    pub fn finish(mut self) -> DomainResult<()> {
        if let Some(mut stdin) = self.stdin.take() {
            stdin.flush().map_err(|e| {
                DomainError::Media(format!("failed flushing ffmpeg stdin pipe: {e}"))
            })?;
            drop(stdin);
        }
        let child = self
            .child
            .take()
            .ok_or_else(|| DomainError::Media("video writer is already finished".into()))?;
        let output = child
            .wait_with_output()
            .map_err(|e| DomainError::Media(format!("failed waiting for ffmpeg writer: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DomainError::Media(format!(
                "ffmpeg video writer exited with {}: {}",
                output.status,
                stderr.trim()
            )));
        }
        Ok(())
    }
}

impl Drop for FrameVideoWriter {
    fn drop(&mut self) {
        drop(self.stdin.take());
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn fnv1a_128_hex(parts: &[&[u8]]) -> String {
    const FNV_OFFSET_BASIS: u128 = 0x6c62272e07bb0142_62b821756295c58d;
    const FNV_PRIME: u128 = 0x0000000001000000_000000000000013b;
    let mut hash = FNV_OFFSET_BASIS;
    for part in parts {
        for &b in *part {
            hash ^= b as u128;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:032x}")
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

    /// Streams sampled RGBA video frames one frame at a time via a piped `ffmpeg` child process,
    /// keeping memory O(1) with respect to video duration.
    pub fn stream_sampled_frames_rgba(
        &self,
        media: &MediaRef,
        sample_fps: f32,
        max_width: u32,
    ) -> DomainResult<SampledFrameStream> {
        if media.width == 0 || media.height == 0 {
            return Err(DomainError::Media("media has no video dimensions".into()));
        }
        if !sample_fps.is_finite() || sample_fps <= 0.0 {
            return Err(DomainError::Media(
                "sample_fps must be positive and finite".into(),
            ));
        }
        let target_w = media.width.min(max_width).max(2) & !1;
        let target_h = (((media.height as f64) * (target_w as f64) / (media.width as f64)).round()
            as u32)
            .max(2)
            & !1;
        let vf = format!("fps={sample_fps:.3},scale={target_w}:{target_h}");

        let mut child = Command::new(&self.ffmpeg_bin)
            .args([
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
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| DomainError::Media(format!("failed to spawn {}: {e}", self.ffmpeg_bin)))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| DomainError::Media("failed to open ffmpeg stdout pipe".into()))?;
        let frame_bytes = (target_w as usize) * (target_h as usize) * 4;
        let frame_step = if media.fps > 0.0 {
            (media.fps / sample_fps).max(1.0)
        } else {
            1.0
        };

        Ok(SampledFrameStream {
            child,
            reader: BufReader::new(stdout),
            width: target_w,
            height: target_h,
            frame_bytes,
            frame_step,
            yielded_count: 0,
            finished: false,
        })
    }

    /// Streams full-resolution, full-fps RGBA video frames one frame at a time for burn-in export.
    pub fn stream_all_frames_rgba(&self, media: &MediaRef) -> DomainResult<SampledFrameStream> {
        if media.width == 0 || media.height == 0 {
            return Err(DomainError::Media("media has no video dimensions".into()));
        }

        let mut child = Command::new(&self.ffmpeg_bin)
            .args([
                "-v",
                "error",
                "-i",
                &media.path,
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "pipe:1",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| DomainError::Media(format!("failed to spawn {}: {e}", self.ffmpeg_bin)))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| DomainError::Media("failed to open ffmpeg stdout pipe".into()))?;
        let frame_bytes = (media.width as usize) * (media.height as usize) * 4;

        Ok(SampledFrameStream {
            child,
            reader: BufReader::new(stdout),
            width: media.width,
            height: media.height,
            frame_bytes,
            frame_step: 1.0,
            yielded_count: 0,
            finished: false,
        })
    }

    /// Extracts sampled RGBA video frames for Vision Privacy Shield keyframe analysis.
    /// Backwards-compatible wrapper that collects from `stream_sampled_frames_rgba`.
    pub fn extract_sampled_frames_rgba(
        &self,
        media: &MediaRef,
        sample_fps: f32,
        max_width: u32,
    ) -> DomainResult<Vec<DecodedFrame>> {
        self.stream_sampled_frames_rgba(media, sample_fps, max_width)?
            .collect()
    }

    /// Generates (and caches by deterministic hash of path + file size + mtime) a
    /// WebView-compatible MP4 preview proxy (`H.264 yuv420p + AAC + faststart`, max width 1280px).
    /// Writes atomically via a temporary file + rename.
    pub fn build_preview_proxy(&self, media: &MediaRef, cache_dir: &Path) -> DomainResult<PathBuf> {
        let meta = std::fs::metadata(&media.path).map_err(|e| {
            DomainError::Media(format!("failed to stat media {}: {e}", media.path))
        })?;
        let file_size = meta.len();
        let mtime_nanos = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        let key = fnv1a_128_hex(&[
            media.path.as_bytes(),
            &file_size.to_le_bytes(),
            &mtime_nanos.to_le_bytes(),
        ]);

        std::fs::create_dir_all(cache_dir).map_err(|e| {
            DomainError::Media(format!(
                "failed to create preview cache dir {}: {e}",
                cache_dir.display()
            ))
        })?;

        let final_path = cache_dir.join(format!("preview_{key}.mp4"));
        if std::fs::metadata(&final_path)
            .map(|m| m.is_file() && m.len() > 0)
            .unwrap_or(false)
        {
            return Ok(final_path);
        }

        let tmp_path = cache_dir.join(format!(".preview_{key}_{}.tmp.mp4", Uuid::new_v4()));
        let tmp_str = tmp_path
            .to_str()
            .ok_or_else(|| DomainError::Media("non-UTF-8 preview proxy temp path".into()))?;

        let output = Command::new(&self.ffmpeg_bin)
            .args([
                "-y",
                "-v",
                "error",
                "-i",
                &media.path,
                "-vf",
                "scale='min(1280,iw)':-2",
                "-pix_fmt",
                "yuv420p",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-crf",
                "24",
                "-movflags",
                "+faststart",
                "-c:a",
                "aac",
                "-b:a",
                "128k",
                "-f",
                "mp4",
                tmp_str,
            ])
            .output()
            .map_err(|e| {
                let _ = std::fs::remove_file(&tmp_path);
                DomainError::Media(format!("failed to spawn {}: {e}", self.ffmpeg_bin))
            })?;

        if !output.status.success() {
            let _ = std::fs::remove_file(&tmp_path);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DomainError::Media(format!(
                "ffmpeg preview proxy exited with {}: {}",
                output.status,
                stderr.trim()
            )));
        }

        std::fs::rename(&tmp_path, &final_path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp_path);
            DomainError::Media(format!(
                "failed to finalize preview proxy {}: {e}",
                final_path.display()
            ))
        })?;

        Ok(final_path)
    }

    /// Computes `num_buckets` normalized peak amplitudes in `[0.0, 1.0]` from 16kHz mono PCM
    /// for real timeline waveform rendering.
    pub fn extract_waveform_peaks(
        &self,
        media: &MediaRef,
        num_buckets: usize,
    ) -> DomainResult<Vec<f32>> {
        if num_buckets == 0 {
            return Ok(Vec::new());
        }
        let pcm = self.extract_pcm_16k(media)?;
        if pcm.is_empty() {
            return Ok(vec![0.0_f32; num_buckets]);
        }

        let len = pcm.len();
        let mut peaks = Vec::with_capacity(num_buckets);
        let mut global_max = 0.0_f32;

        for b in 0..num_buckets {
            let start = (b * len) / num_buckets;
            let mut end = ((b + 1) * len) / num_buckets;
            if end <= start {
                end = (start + 1).min(len);
            }
            let peak = pcm[start..end]
                .iter()
                .fold(0.0_f32, |acc, &sample| acc.max(sample.abs()));
            if peak > global_max {
                global_max = peak;
            }
            peaks.push(peak);
        }

        if global_max > 0.0 {
            for p in &mut peaks {
                *p = (*p / global_max).clamp(0.0, 1.0);
            }
        }

        Ok(peaks)
    }

    /// Opens a streaming `FrameVideoWriter` configured with this extractor's `ffmpeg` binary.
    pub fn open_video_writer(
        &self,
        width: u32,
        height: u32,
        fps: f32,
        audio_source: Option<&Path>,
        output_path: &Path,
    ) -> DomainResult<FrameVideoWriter> {
        FrameVideoWriter::new(
            &self.ffmpeg_bin,
            width,
            height,
            fps,
            audio_source,
            output_path,
        )
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

    fn generate_test_video(path: &Path, width: u32, height: u32, fps: u32, duration_s: u32) {
        let src = format!("testsrc=duration={duration_s}:size={width}x{height}:rate={fps}");
        let status = Command::new("ffmpeg")
            .args([
                "-y",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                &src,
                "-f",
                "lavfi",
                "-i",
                &format!("sine=frequency=440:duration={duration_s}"),
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-shortest",
                path.to_str().unwrap(),
            ])
            .status()
            .expect("ffmpeg must be installed to generate test video");
        assert!(status.success(), "ffmpeg failed to generate test video");
    }

    #[test]
    fn stream_sampled_and_all_frames_from_generated_video() {
        let tmp_mp4 = std::env::temp_dir().join(format!("tinycut_stream_{}.mp4", Uuid::new_v4()));
        generate_test_video(&tmp_mp4, 160, 120, 10, 1);

        let extractor = FfmpegMediaExtractor::default();
        let media = extractor.probe_media(&tmp_mp4).expect("probe should succeed");
        assert_eq!(media.width, 160);
        assert_eq!(media.height, 120);

        // Stream sampled frames at 5fps, max width 80
        let stream = extractor
            .stream_sampled_frames_rgba(&media, 5.0, 80)
            .expect("stream_sampled_frames_rgba should succeed");
        assert_eq!(stream.width(), 80);
        assert_eq!(stream.height(), 60);

        let mut streamed_count = 0usize;
        for frame_res in stream {
            let frame = frame_res.expect("frame decode should succeed");
            assert_eq!(frame.width, 80);
            assert_eq!(frame.height, 60);
            assert_eq!(frame.rgba.len(), 80 * 60 * 4);
            streamed_count += 1;
        }
        assert!(
            (4..=6).contains(&streamed_count),
            "expected ~5 sampled frames, got {streamed_count}"
        );

        // Verify backwards-compatible extract_sampled_frames_rgba collects the same count
        let collected = extractor
            .extract_sampled_frames_rgba(&media, 5.0, 80)
            .expect("extract_sampled_frames_rgba should succeed");
        assert_eq!(collected.len(), streamed_count);

        // Verify early drop cleanly terminates child process
        {
            let mut early_stream = extractor
                .stream_all_frames_rgba(&media)
                .expect("stream_all_frames_rgba should succeed");
            let first = early_stream
                .next()
                .expect("should yield first frame")
                .expect("first frame should decode");
            assert_eq!(first.frame_index, 0);
            assert_eq!(first.width, 160);
            assert_eq!(first.height, 120);
            assert_eq!(first.rgba.len(), 160 * 120 * 4);
        }

        // Verify full-fps stream yields all frames
        let all_frames: Vec<DecodedFrame> = extractor
            .stream_all_frames_rgba(&media)
            .expect("stream_all_frames_rgba should succeed")
            .collect::<DomainResult<Vec<_>>>()
            .expect("all frames should decode");
        assert!(
            (8..=12).contains(&all_frames.len()),
            "expected ~10 full frames, got {}",
            all_frames.len()
        );

        let _ = std::fs::remove_file(&tmp_mp4);
    }

    #[test]
    fn extract_waveform_peaks_returns_expected_length_and_range() {
        let tmp_wav = std::env::temp_dir().join(format!("tinycut_peaks_{}.wav", Uuid::new_v4()));
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
            .expect("ffmpeg must be installed");
        assert!(status.success());

        let extractor = FfmpegMediaExtractor::default();
        let media = extractor.probe_media(&tmp_wav).expect("probe should succeed");

        let empty = extractor
            .extract_waveform_peaks(&media, 0)
            .expect("0 buckets should succeed");
        assert!(empty.is_empty());

        let peaks = extractor
            .extract_waveform_peaks(&media, 64)
            .expect("waveform peaks should succeed");
        assert_eq!(peaks.len(), 64);
        assert!(peaks.iter().all(|&p| (0.0..=1.0).contains(&p)));
        let max_peak = peaks.iter().copied().fold(0.0_f32, f32::max);
        assert!((max_peak - 1.0).abs() < 1e-4, "expected normalized max 1.0, got {max_peak}");

        let _ = std::fs::remove_file(&tmp_wav);
    }

    #[test]
    fn build_preview_proxy_creates_and_caches_valid_mp4() {
        let tmp_dir = std::env::temp_dir().join(format!("tinycut_proxy_test_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&tmp_dir).unwrap();
        let src_mp4 = tmp_dir.join("source.mp4");
        let cache_dir = tmp_dir.join("cache");
        generate_test_video(&src_mp4, 320, 240, 15, 1);

        let extractor = FfmpegMediaExtractor::default();
        let media = extractor.probe_media(&src_mp4).expect("probe should succeed");

        let proxy_path = extractor
            .build_preview_proxy(&media, &cache_dir)
            .expect("build_preview_proxy should succeed");
        assert!(proxy_path.exists());
        assert_eq!(proxy_path.extension().and_then(|e| e.to_str()), Some("mp4"));

        let probed_proxy = extractor
            .probe_media(&proxy_path)
            .expect("proxy mp4 should be valid and probeable");
        assert_eq!(probed_proxy.width, 320);
        assert_eq!(probed_proxy.height, 240);
        assert!(probed_proxy.duration_ms >= 800);

        // Second call should hit the deterministic cache and return the exact same file untouched
        let mtime_before = std::fs::metadata(&proxy_path).unwrap().modified().unwrap();
        let proxy_path_2 = extractor
            .build_preview_proxy(&media, &cache_dir)
            .expect("cached build_preview_proxy should succeed");
        assert_eq!(proxy_path, proxy_path_2);
        let mtime_after = std::fs::metadata(&proxy_path_2).unwrap().modified().unwrap();
        assert_eq!(mtime_before, mtime_after);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn frame_video_writer_produces_valid_mp4_from_synthetic_frames() {
        let tmp_dir = std::env::temp_dir().join(format!("tinycut_writer_test_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&tmp_dir).unwrap();
        let out_mp4 = tmp_dir.join("rendered.mp4");

        let w = 64_u32;
        let h = 48_u32;
        let fps = 10.0_f32;
        let extractor = FfmpegMediaExtractor::default();
        let mut writer = extractor
            .open_video_writer(w, h, fps, None, &out_mp4)
            .expect("open_video_writer should succeed");

        let frame_bytes = (w * h * 4) as usize;
        for i in 0..10_u8 {
            let mut frame = vec![0u8; frame_bytes];
            for px in frame.as_chunks_mut::<4>().0 {
                px[0] = i.saturating_mul(25);
                px[1] = 128;
                px[2] = 255 - i.saturating_mul(25);
                px[3] = 255;
            }
            writer
                .write_frame_rgba(&frame)
                .expect("write_frame_rgba should succeed");
        }
        writer.finish().expect("writer finish should succeed");

        assert!(out_mp4.exists());
        let probed = extractor
            .probe_media(&out_mp4)
            .expect("written mp4 should be valid and probeable");
        assert_eq!(probed.width, w);
        assert_eq!(probed.height, h);
        assert!(probed.duration_ms >= 800 && probed.duration_ms <= 1200);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
