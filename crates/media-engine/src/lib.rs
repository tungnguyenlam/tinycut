use tinycut_core_domain::{DomainResult, MediaRef};

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

/// Smart Proxy Offloading & local media extraction interface.
/// Native implementations will bind AVFoundation (Apple), MediaExtractor (Android),
/// and symphonia / LGPL FFmpeg (Desktop).
pub trait MediaExtractor: Send + Sync {
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
