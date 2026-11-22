use tinycut_core_domain::{RedactionKeyframe, RedactionTrack, ShieldConfig};

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

#[cfg(test)]
mod tests {
    use super::*;
    use tinycut_core_domain::{NormalizedBox, RedactionKind};

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
                    y: 0.4,
                    w: 0.1,
                    h: 0.05,
                },
                confidence: 0.95,
            }],
        };
        let padded = apply_zero_leak_padding(&track, &ShieldConfig::default(), 100);
        // 3 frames before + original + 3 frames after = 7 keyframes
        assert_eq!(padded.keyframes.len(), 7);
        assert_eq!(padded.keyframes.first().unwrap().frame_index, 7);
        assert_eq!(padded.keyframes.last().unwrap().frame_index, 13);
        assert!(padded.keyframes[0].bbox.w > 0.1);
    }
}
