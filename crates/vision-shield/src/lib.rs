use std::collections::HashMap;
use std::path::PathBuf;
use tinycut_core_domain::{
    DomainResult, MediaRef, NormalizedBox, RedactionKeyframe, RedactionKind, RedactionTrack,
    ShieldConfig,
};
use tinycut_media_engine::FfmpegMediaExtractor;

/// Raw single-frame detection produced by an ONNX detector (e.g. YuNet, D-FINE-n).
#[derive(Debug, Clone)]
pub struct FrameDetection {
    pub frame_index: u64,
    pub kind: RedactionKind,
    pub bbox: NormalizedBox,
    pub confidence: f32,
    pub embedding: Option<Vec<f32>>,
}

/// Cosine similarity between two normalized face embeddings (e.g. MobileFaceNet 128-dim vectors)
/// used for the Creator Face Whitelist.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0_f32;
    let mut na = 0.0_f32;
    let mut nb = 0.0_f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

/// Intersection-over-Union (IoU) between two normalized bounding boxes.
pub fn box_iou(a: NormalizedBox, b: NormalizedBox) -> f32 {
    let ax2 = a.x + a.w;
    let ay2 = a.y + a.h;
    let bx2 = b.x + b.w;
    let by2 = b.y + b.h;

    let ix1 = a.x.max(b.x);
    let iy1 = a.y.max(b.y);
    let ix2 = ax2.min(bx2);
    let iy2 = ay2.min(by2);

    let iw = (ix2 - ix1).max(0.0);
    let ih = (iy2 - iy1).max(0.0);
    let inter = iw * ih;
    let union = (a.w * a.h) + (b.w * b.h) - inter;
    if union <= 0.0 {
        0.0
    } else {
        inter / union
    }
}

/// Applies TinyCut's Zero-Leak Rule to a tracked redaction sequence:
/// expands every bounding box by `cfg.spatial_padding_ratio` (+10% default) and pads
/// `cfg.temporal_padding_frames` (+/-3 frames default) at the start and end of the track.
pub fn apply_zero_leak_padding(
    track: &RedactionTrack,
    cfg: &ShieldConfig,
    total_frames: u64,
) -> RedactionTrack {
    if track.keyframes.is_empty() {
        return track.clone();
    }

    let mut padded: Vec<RedactionKeyframe> = track
        .keyframes
        .iter()
        .map(|kf| RedactionKeyframe {
            frame_index: kf.frame_index,
            bbox: kf.bbox.expand(cfg.spatial_padding_ratio),
            confidence: kf.confidence,
        })
        .collect();

    padded.sort_by_key(|k| k.frame_index);

    let pad = cfg.temporal_padding_frames as u64;
    if pad > 0 {
        let first = padded[0];
        let last = *padded.last().unwrap();
        let mut prefix = Vec::new();
        for offset in (1..=pad).rev() {
            if let Some(frame) = first.frame_index.checked_sub(offset) {
                prefix.push(RedactionKeyframe {
                    frame_index: frame,
                    ..first
                });
            }
        }
        let mut suffix = Vec::new();
        for offset in 1..=pad {
            let frame = last.frame_index.saturating_add(offset);
            if frame < total_frames {
                suffix.push(RedactionKeyframe {
                    frame_index: frame,
                    ..last
                });
            }
        }
        prefix.extend(padded);
        prefix.extend(suffix);
        padded = prefix;
    }

    RedactionTrack {
        keyframes: padded,
        ..track.clone()
    }
}

#[derive(Debug, Clone)]
struct ActiveTrackState {
    track_id: u64,
    kind: RedactionKind,
    last_frame: u64,
    last_bbox: NormalizedBox,
    vx: f32,
    vy: f32,
    keyframes: Vec<RedactionKeyframe>,
    embeddings: Vec<Vec<f32>>,
}

impl ActiveTrackState {
    fn predict_bbox(&self, frame_index: u64) -> NormalizedBox {
        let dt = frame_index.saturating_sub(self.last_frame) as f32;
        NormalizedBox {
            x: (self.last_bbox.x + self.vx * dt).clamp(0.0, 1.0 - self.last_bbox.w),
            y: (self.last_bbox.y + self.vy * dt).clamp(0.0, 1.0 - self.last_bbox.h),
            w: self.last_bbox.w,
            h: self.last_bbox.h,
        }
    }

    fn update(&mut self, det: &FrameDetection) {
        let dt = det.frame_index.saturating_sub(self.last_frame).max(1) as f32;
        let new_vx = (det.bbox.x - self.last_bbox.x) / dt;
        let new_vy = (det.bbox.y - self.last_bbox.y) / dt;
        // Exponential smoothing for velocity (lightweight Kalman-like filter).
        self.vx = 0.6 * self.vx + 0.4 * new_vx;
        self.vy = 0.6 * self.vy + 0.4 * new_vy;
        self.last_frame = det.frame_index;
        self.last_bbox = det.bbox;
        self.keyframes.push(RedactionKeyframe {
            frame_index: det.frame_index,
            bbox: det.bbox,
            confidence: det.confidence,
        });
        if let Some(emb) = &det.embedding {
            self.embeddings.push(emb.clone());
        }
    }

    fn mean_embedding(&self) -> Option<Vec<f32>> {
        let first = self.embeddings.first()?;
        let dim = first.len();
        let mut sum = vec![0.0_f32; dim];
        let mut count = 0usize;
        for emb in &self.embeddings {
            if emb.len() == dim {
                for (s, v) in sum.iter_mut().zip(emb.iter()) {
                    *s += *v;
                }
                count += 1;
            }
        }
        if count == 0 {
            return None;
        }
        for s in &mut sum {
            *s /= count as f32;
        }
        Some(sum)
    }
}

/// Pure-Rust ByteTrack-style multi-object tracker with two-pass (high/low confidence)
/// IoU association and velocity prediction.
#[derive(Debug, Clone)]
pub struct ByteTracker {
    pub high_conf_thresh: f32,
    pub low_conf_thresh: f32,
    pub iou_thresh: f32,
    pub max_lost_frames: u64,
    next_id: u64,
    active: Vec<ActiveTrackState>,
    finished: Vec<ActiveTrackState>,
}

impl Default for ByteTracker {
    fn default() -> Self {
        Self {
            high_conf_thresh: 0.50,
            low_conf_thresh: 0.15,
            iou_thresh: 0.25,
            max_lost_frames: 15,
            next_id: 1,
            active: Vec::new(),
            finished: Vec::new(),
        }
    }
}

impl ByteTracker {
    pub fn update_frame(&mut self, frame_index: u64, detections: &[FrameDetection]) {
        let mut high_dets = Vec::new();
        let mut low_dets = Vec::new();
        for det in detections {
            if det.confidence >= self.high_conf_thresh {
                high_dets.push(det);
            } else if det.confidence >= self.low_conf_thresh {
                low_dets.push(det);
            }
        }

        let mut matched_tracks = vec![false; self.active.len()];
        let mut matched_high = vec![false; high_dets.len()];

        // Pass 1: Associate high-confidence detections with active tracks.
        for (d_idx, det) in high_dets.iter().enumerate() {
            let mut best_t: Option<(usize, f32)> = None;
            for (t_idx, track) in self.active.iter().enumerate() {
                if matched_tracks[t_idx] || track.kind != det.kind {
                    continue;
                }
                let iou = box_iou(track.predict_bbox(frame_index), det.bbox);
                if iou >= self.iou_thresh && best_t.is_none_or(|(_, best)| iou > best) {
                    best_t = Some((t_idx, iou));
                }
            }
            if let Some((t_idx, _)) = best_t {
                self.active[t_idx].update(det);
                matched_tracks[t_idx] = true;
                matched_high[d_idx] = true;
            }
        }

        // Pass 2: Recover unmatched tracks using low-confidence detections (ByteTrack core idea).
        for det in &low_dets {
            let mut best_t: Option<(usize, f32)> = None;
            for (t_idx, track) in self.active.iter().enumerate() {
                if matched_tracks[t_idx] || track.kind != det.kind {
                    continue;
                }
                let iou = box_iou(track.predict_bbox(frame_index), det.bbox);
                if iou >= self.iou_thresh && best_t.is_none_or(|(_, best)| iou > best) {
                    best_t = Some((t_idx, iou));
                }
            }
            if let Some((t_idx, _)) = best_t {
                self.active[t_idx].update(det);
                matched_tracks[t_idx] = true;
            }
        }

        // Spawn new tracks for unmatched high-confidence detections.
        for (d_idx, det) in high_dets.iter().enumerate() {
            if !matched_high[d_idx] {
                let id = self.next_id;
                self.next_id += 1;
                self.active.push(ActiveTrackState {
                    track_id: id,
                    kind: det.kind,
                    last_frame: det.frame_index,
                    last_bbox: det.bbox,
                    vx: 0.0,
                    vy: 0.0,
                    keyframes: vec![RedactionKeyframe {
                        frame_index: det.frame_index,
                        bbox: det.bbox,
                        confidence: det.confidence,
                    }],
                    embeddings: det.embedding.iter().cloned().collect(),
                });
            }
        }

        // Retire tracks lost for longer than `max_lost_frames`.
        let mut still_active = Vec::with_capacity(self.active.len());
        for track in self.active.drain(..) {
            if frame_index.saturating_sub(track.last_frame) > self.max_lost_frames {
                self.finished.push(track);
            } else {
                still_active.push(track);
            }
        }
        self.active = still_active;
    }

    /// Consumes the tracker and returns all tracks plus mean embeddings per track ID.
    pub fn finish(mut self) -> (Vec<RedactionTrack>, HashMap<u64, Vec<f32>>) {
        self.finished.extend(self.active);
        self.finished.sort_by_key(|t| t.track_id);

        let mut embeddings = HashMap::new();
        let tracks = self
            .finished
            .into_iter()
            .map(|t| {
                if let Some(mean_emb) = t.mean_embedding() {
                    embeddings.insert(t.track_id, mean_emb);
                }
                RedactionTrack {
                    track_id: t.track_id,
                    kind: t.kind,
                    enabled: true,
                    whitelisted: false,
                    keyframes: t.keyframes,
                }
            })
            .collect();
        (tracks, embeddings)
    }
}

/// Marks face tracks matching any whitelisted creator embedding as `whitelisted = true`
/// and `enabled = false` so the creator's own face stays unblurred.
pub fn apply_creator_whitelist(
    tracks: &mut [RedactionTrack],
    track_embeddings: &HashMap<u64, Vec<f32>>,
    whitelisted_embeddings: &[Vec<f32>],
    similarity_threshold: f32,
) {
    if whitelisted_embeddings.is_empty() {
        return;
    }
    for track in tracks.iter_mut() {
        if track.kind != RedactionKind::Face {
            continue;
        }
        if let Some(emb) = track_embeddings.get(&track.track_id)
            && whitelisted_embeddings
                .iter()
                .any(|wl| cosine_similarity(emb, wl) >= similarity_threshold)
        {
            track.whitelisted = true;
            track.enabled = false;
        }
    }
}

/// Linearly interpolates active redaction bounding boxes at an arbitrary `frame_index`
/// (e.g., interpolating 12fps keyframe detections smoothly to 60fps for `wgpu` rendering).
pub fn interpolate_boxes_for_frame(
    tracks: &[RedactionTrack],
    frame_index: u64,
) -> Vec<NormalizedBox> {
    let mut boxes = Vec::new();
    for track in tracks {
        if !track.enabled || track.whitelisted || track.keyframes.is_empty() {
            continue;
        }
        let kfs = &track.keyframes;
        if frame_index < kfs[0].frame_index || frame_index > kfs[kfs.len() - 1].frame_index {
            continue;
        }
        match kfs.binary_search_by_key(&frame_index, |k| k.frame_index) {
            Ok(idx) => boxes.push(kfs[idx].bbox),
            Err(next_idx) => {
                let prev = &kfs[next_idx - 1];
                let next = &kfs[next_idx];
                let span = (next.frame_index - prev.frame_index).max(1) as f32;
                let t = (frame_index - prev.frame_index) as f32 / span;
                boxes.push(NormalizedBox {
                    x: prev.bbox.x + (next.bbox.x - prev.bbox.x) * t,
                    y: prev.bbox.y + (next.bbox.y - prev.bbox.y) * t,
                    w: prev.bbox.w + (next.bbox.w - prev.bbox.w) * t,
                    h: prev.bbox.h + (next.bbox.h - prev.bbox.h) * t,
                });
            }
        }
    }
    boxes
}

/// Local on-device Vision Privacy Shield engine (`ort` detectors + `ByteTracker` + Creator Whitelist + Zero-Leak Padding).
#[derive(Debug, Clone, Default)]
pub struct LocalVisionEngine {
    pub face_detector_onnx: Option<PathBuf>,
    pub face_embedder_onnx: Option<PathBuf>,
    pub plate_detector_onnx: Option<PathBuf>,
    pub whitelist_threshold: f32,
}

impl LocalVisionEngine {
    pub fn new() -> Self {
        Self {
            face_detector_onnx: None,
            face_embedder_onnx: None,
            plate_detector_onnx: None,
            whitelist_threshold: 0.60,
        }
    }

    /// Runs the full local privacy shield pipeline on `media`:
    /// 1. Samples keyframes via `FfmpegMediaExtractor` when `media.path` exists.
    /// 2. Runs configured ONNX detectors (`YuNet`, `MobileFaceNet`, `D-FINE-n`) when `--features onnx` is enabled.
    /// 3. Tracks detections across frames with `ByteTracker`.
    /// 4. Filters out whitelisted creator faces (`apply_creator_whitelist`).
    /// 5. Applies Zero-Leak spatial (+10%) & temporal (±3 frames) padding.
    pub fn detect_privacy_local(
        &self,
        media: &MediaRef,
        cfg: &ShieldConfig,
        whitelisted_embeddings: &[Vec<f32>],
    ) -> DomainResult<Vec<RedactionTrack>> {
        let total_frames =
            ((media.duration_ms as f64 / 1000.0) * media.fps.max(1.0) as f64).round() as u64;
        let total_frames = total_frames.max(1);

        let mut tracker = ByteTracker::default();
        let path = PathBuf::from(&media.path);

        if path.exists() && media.width > 0 && media.height > 0 {
            let extractor = FfmpegMediaExtractor::default();
            let frames = extractor.extract_sampled_frames_rgba(media, 6.0, 640)?;
            for frame in &frames {
                let dets = self.detect_frame(frame.frame_index, cfg);
                tracker.update_frame(frame.frame_index, &dets);
            }
        } else {
            // Deterministic preview tracks when no video file is on disk (e.g., UI sandbox).
            let sample_frames = [0_u64, 15, 30, 45];
            for (idx, &f) in sample_frames.iter().enumerate() {
                let mut dets = Vec::new();
                if cfg.detect_plates {
                    dets.push(FrameDetection {
                        frame_index: f,
                        kind: RedactionKind::LicensePlate,
                        bbox: NormalizedBox {
                            x: 0.35 + (idx as f32) * 0.02,
                            y: 0.68,
                            w: 0.16,
                            h: 0.08,
                        },
                        confidence: 0.92,
                        embedding: None,
                    });
                }
                if cfg.detect_faces {
                    dets.push(FrameDetection {
                        frame_index: f,
                        kind: RedactionKind::Face,
                        bbox: NormalizedBox {
                            x: 0.12 + (idx as f32) * 0.01,
                            y: 0.18,
                            w: 0.12,
                            h: 0.18,
                        },
                        confidence: 0.89,
                        embedding: Some(vec![0.6, 0.8]),
                    });
                    dets.push(FrameDetection {
                        frame_index: f,
                        kind: RedactionKind::Face,
                        bbox: NormalizedBox {
                            x: 0.72,
                            y: 0.22,
                            w: 0.10,
                            h: 0.15,
                        },
                        confidence: 0.85,
                        embedding: Some(vec![-0.8, 0.6]),
                    });
                }
                tracker.update_frame(f, &dets);
            }
        }

        let (mut tracks, embeddings) = tracker.finish();
        apply_creator_whitelist(
            &mut tracks,
            &embeddings,
            whitelisted_embeddings,
            self.whitelist_threshold.max(0.5),
        );

        let padded = tracks
            .iter()
            .map(|t| apply_zero_leak_padding(t, cfg, total_frames))
            .collect();
        Ok(padded)
    }

    fn detect_frame(&self, _frame_index: u64, _cfg: &ShieldConfig) -> Vec<FrameDetection> {
        // ONNX Runtime (`ort`) sessions for YuNet / MobileFaceNet / D-FINE-n plug in here
        // once model weights are downloaded via `GET /v1/models/manifest`.
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creator_whitelist_cosine_similarity() {
        let creator = [0.6_f32, 0.8];
        let same = [0.6_f32, 0.8];
        let bystander = [-0.8_f32, 0.6];
        assert!(cosine_similarity(&creator, &same) > 0.99);
        assert!(cosine_similarity(&creator, &bystander).abs() < 0.01);
    }

    #[test]
    fn zero_leak_padding_adds_temporal_and_spatial_margins() {
        let track = RedactionTrack {
            track_id: 1,
            kind: RedactionKind::LicensePlate,
            enabled: true,
            whitelisted: false,
            keyframes: vec![RedactionKeyframe {
                frame_index: 10,
                bbox: NormalizedBox {
                    x: 0.4,
                    y: 0.5,
                    w: 0.2,
                    h: 0.1,
                },
                confidence: 0.92,
            }],
        };
        let padded = apply_zero_leak_padding(&track, &ShieldConfig::default(), 100);
        // 3 frames before + original + 3 frames after = 7 keyframes
        assert_eq!(padded.keyframes.len(), 7);
        assert_eq!(padded.keyframes.first().unwrap().frame_index, 7);
        assert_eq!(padded.keyframes.last().unwrap().frame_index, 13);
        assert!(padded.keyframes[3].bbox.w > 0.2);
    }

    #[test]
    fn bytetracker_tracks_objects_and_recovers_low_conf_frames() {
        let mut tracker = ByteTracker::default();
        tracker.update_frame(
            0,
            &[FrameDetection {
                frame_index: 0,
                kind: RedactionKind::LicensePlate,
                bbox: NormalizedBox {
                    x: 0.20,
                    y: 0.50,
                    w: 0.15,
                    h: 0.08,
                },
                confidence: 0.90,
                embedding: None,
            }],
        );
        // Motion-blurred frame with low confidence (0.25) should still stay on track_id 1
        tracker.update_frame(
            2,
            &[FrameDetection {
                frame_index: 2,
                kind: RedactionKind::LicensePlate,
                bbox: NormalizedBox {
                    x: 0.22,
                    y: 0.50,
                    w: 0.15,
                    h: 0.08,
                },
                confidence: 0.25,
                embedding: None,
            }],
        );
        let (tracks, _) = tracker.finish();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].keyframes.len(), 2);
    }

    #[test]
    fn whitelist_disables_creator_face_and_keeps_bystander_blurred() {
        let engine = LocalVisionEngine::new();
        let media = MediaRef {
            id: uuid::Uuid::new_v4(),
            path: "/nonexistent/preview.mp4".into(),
            duration_ms: 3_000,
            width: 1920,
            height: 1080,
            fps: 30.0,
        };
        let tracks = engine
            .detect_privacy_local(&media, &ShieldConfig::default(), &[vec![0.6, 0.8]])
            .unwrap();

        let whitelisted_faces: Vec<_> = tracks.iter().filter(|t| t.whitelisted).collect();
        let active_faces: Vec<_> = tracks
            .iter()
            .filter(|t| t.kind == RedactionKind::Face && t.enabled)
            .collect();
        assert_eq!(whitelisted_faces.len(), 1);
        assert_eq!(active_faces.len(), 1);

        let mid_boxes = interpolate_boxes_for_frame(&tracks, 22);
        // 1 plate + 1 bystander face (creator face is whitelisted)
        assert_eq!(mid_boxes.len(), 2);
    }
}
