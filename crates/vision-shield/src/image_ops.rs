use std::collections::VecDeque;
use tinycut_core_domain::NormalizedBox;

use crate::{FrameDetection, box_iou};

/// Bilinearly resizes an RGBA8 buffer from `(src_w, src_h)` to `(dst_w, dst_h)`.
pub fn resize_rgba_bilinear(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
) -> Vec<u8> {
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return Vec::new();
    }
    if src_w == dst_w && src_h == dst_h && src.len() >= (src_w * src_h * 4) as usize {
        return src[..(src_w * src_h * 4) as usize].to_vec();
    }

    let mut out = vec![0u8; (dst_w * dst_h * 4) as usize];
    let scale_x = src_w as f32 / dst_w as f32;
    let scale_y = src_h as f32 / dst_h as f32;

    for dy in 0..dst_h {
        let sy = ((dy as f32 + 0.5) * scale_y - 0.5).clamp(0.0, (src_h - 1) as f32);
        let y0 = sy.floor() as u32;
        let y1 = (y0 + 1).min(src_h - 1);
        let wy = sy - y0 as f32;

        for dx in 0..dst_w {
            let sx = ((dx as f32 + 0.5) * scale_x - 0.5).clamp(0.0, (src_w - 1) as f32);
            let x0 = sx.floor() as u32;
            let x1 = (x0 + 1).min(src_w - 1);
            let wx = sx - x0 as f32;

            let i00 = ((y0 * src_w + x0) * 4) as usize;
            let i10 = ((y0 * src_w + x1) * 4) as usize;
            let i01 = ((y1 * src_w + x0) * 4) as usize;
            let i11 = ((y1 * src_w + x1) * 4) as usize;
            let dst_idx = ((dy * dst_w + dx) * 4) as usize;

            for c in 0..4 {
                let top = src[i00 + c] as f32 * (1.0 - wx) + src[i10 + c] as f32 * wx;
                let bot = src[i01 + c] as f32 * (1.0 - wx) + src[i11 + c] as f32 * wx;
                out[dst_idx + c] = (top * (1.0 - wy) + bot * wy).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// Crops an RGBA8 sub-region described by `bbox` (clamped to image bounds).
/// Returns `(crop_w, crop_h, crop_rgba)`.
pub fn crop_rgba(src: &[u8], src_w: u32, src_h: u32, bbox: NormalizedBox) -> (u32, u32, Vec<u8>) {
    if src_w == 0 || src_h == 0 {
        return (0, 0, Vec::new());
    }
    let x1 = ((bbox.x.clamp(0.0, 1.0) * src_w as f32).floor() as u32).min(src_w - 1);
    let y1 = ((bbox.y.clamp(0.0, 1.0) * src_h as f32).floor() as u32).min(src_h - 1);
    let x2 = (((bbox.x + bbox.w).clamp(0.0, 1.0) * src_w as f32).ceil() as u32)
        .clamp(x1 + 1, src_w);
    let y2 = (((bbox.y + bbox.h).clamp(0.0, 1.0) * src_h as f32).ceil() as u32)
        .clamp(y1 + 1, src_h);

    let cw = x2 - x1;
    let ch = y2 - y1;
    let mut out = vec![0u8; (cw * ch * 4) as usize];
    for y in 0..ch {
        let src_row = (((y1 + y) * src_w + x1) * 4) as usize;
        let dst_row = ((y * cw) * 4) as usize;
        let len = (cw * 4) as usize;
        out[dst_row..dst_row + len].copy_from_slice(&src[src_row..src_row + len]);
    }
    (cw, ch, out)
}

/// Converts RGBA8 `(w, h)` to NCHW float32 BGR in `[0.0, 255.0]` (expected by OpenCV Zoo YuNet).
pub fn rgba_to_nchw_bgr_255(rgba: &[u8], w: u32, h: u32) -> Vec<f32> {
    let hw = (w * h) as usize;
    let mut tensor = vec![0.0_f32; 3 * hw];
    for i in 0..hw {
        let off = i * 4;
        tensor[i] = rgba[off + 2] as f32; // B
        tensor[hw + i] = rgba[off + 1] as f32; // G
        tensor[2 * hw + i] = rgba[off] as f32; // R
    }
    tensor
}

/// Converts RGBA8 `(w, h)` to NCHW float32 RGB in `[0.0, 1.0]` (expected by RT-DETRv2).
pub fn rgba_to_nchw_rgb_01(rgba: &[u8], w: u32, h: u32) -> Vec<f32> {
    let hw = (w * h) as usize;
    let mut tensor = vec![0.0_f32; 3 * hw];
    for i in 0..hw {
        let off = i * 4;
        tensor[i] = rgba[off] as f32 / 255.0;
        tensor[hw + i] = rgba[off + 1] as f32 / 255.0;
        tensor[2 * hw + i] = rgba[off + 2] as f32 / 255.0;
    }
    tensor
}

/// Converts RGBA8 `(w, h)` to NCHW float32 RGB in `[-1.0, 1.0]` (expected by SFace & PP-OCR rec).
pub fn rgba_to_nchw_rgb_neg1_1(rgba: &[u8], w: u32, h: u32) -> Vec<f32> {
    let hw = (w * h) as usize;
    let mut tensor = vec![0.0_f32; 3 * hw];
    for i in 0..hw {
        let off = i * 4;
        tensor[i] = (rgba[off] as f32 - 127.5) / 127.5;
        tensor[hw + i] = (rgba[off + 1] as f32 - 127.5) / 127.5;
        tensor[2 * hw + i] = (rgba[off + 2] as f32 - 127.5) / 127.5;
    }
    tensor
}

/// Converts RGBA8 `(w, h)` to NCHW float32 RGB normalized with ImageNet mean/std (PP-OCRv4 det).
pub fn rgba_to_nchw_rgb_imagenet(rgba: &[u8], w: u32, h: u32) -> Vec<f32> {
    const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
    const STD: [f32; 3] = [0.229, 0.224, 0.225];
    let hw = (w * h) as usize;
    let mut tensor = vec![0.0_f32; 3 * hw];
    for i in 0..hw {
        let off = i * 4;
        tensor[i] = (rgba[off] as f32 / 255.0 - MEAN[0]) / STD[0];
        tensor[hw + i] = (rgba[off + 1] as f32 / 255.0 - MEAN[1]) / STD[1];
        tensor[2 * hw + i] = (rgba[off + 2] as f32 / 255.0 - MEAN[2]) / STD[2];
    }
    tensor
}

/// Converts RGBA8 `(w, h)` to 8-bit grayscale luma (used by QR code detection).
pub fn rgba_to_luma8(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let hw = (w * h) as usize;
    let mut luma = Vec::with_capacity(hw);
    for i in 0..hw {
        let off = i * 4;
        let r = rgba[off] as u32;
        let g = rgba[off + 1] as u32;
        let b = rgba[off + 2] as u32;
        luma.push(((r * 77 + g * 150 + b * 29) >> 8) as u8);
    }
    luma
}

/// L2-normalizes a feature embedding vector in place.
pub fn l2_normalize(vec: &mut [f32]) {
    let norm = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 1e-12 {
        for v in vec.iter_mut() {
            *v /= norm;
        }
    }
}

/// Greedy Non-Maximum Suppression sorted by descending confidence.
pub fn nms_detections(mut dets: Vec<FrameDetection>, iou_threshold: f32) -> Vec<FrameDetection> {
    dets.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut kept: Vec<FrameDetection> = Vec::new();
    for det in dets {
        let suppressed = kept
            .iter()
            .any(|k| k.kind == det.kind && box_iou(k.bbox, det.bbox) > iou_threshold);
        if !suppressed {
            kept.push(det);
        }
    }
    kept
}

/// Pure-Rust connected-component box extraction over a DBNet probability map `[H * W]`.
/// Returns `(NormalizedBox, mean_score)` pairs expanded slightly for DBNet unclip margin.
pub fn extract_text_boxes_from_prob_map(
    prob_map: &[f32],
    w: usize,
    h: usize,
    bin_thresh: f32,
    box_thresh: f32,
) -> Vec<(NormalizedBox, f32)> {
    if w == 0 || h == 0 || prob_map.len() < w * h {
        return Vec::new();
    }
    let mut visited = vec![false; w * h];
    let mut queue = VecDeque::new();
    let mut boxes = Vec::new();

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if visited[idx] || prob_map[idx] < bin_thresh {
                continue;
            }
            visited[idx] = true;
            queue.push_back((x, y));

            let mut min_x = x;
            let mut max_x = x;
            let mut min_y = y;
            let mut max_y = y;
            let mut count = 0usize;
            let mut sum_prob = 0.0_f32;

            while let Some((cx, cy)) = queue.pop_front() {
                let c_idx = cy * w + cx;
                count += 1;
                sum_prob += prob_map[c_idx];
                min_x = min_x.min(cx);
                max_x = max_x.max(cx);
                min_y = min_y.min(cy);
                max_y = max_y.max(cy);

                for (nx, ny) in [
                    (cx.wrapping_sub(1), cy),
                    (cx + 1, cy),
                    (cx, cy.wrapping_sub(1)),
                    (cx, cy + 1),
                ] {
                    if nx < w && ny < h {
                        let n_idx = ny * w + nx;
                        if !visited[n_idx] && prob_map[n_idx] >= bin_thresh {
                            visited[n_idx] = true;
                            queue.push_back((nx, ny));
                        }
                    }
                }
            }

            if count < 12 {
                continue;
            }
            let mean_score = sum_prob / count as f32;
            if mean_score < box_thresh {
                continue;
            }

            let bw = (max_x - min_x + 1) as f32;
            let bh = (max_y - min_y + 1) as f32;
            if bw < 6.0 || bh < 4.0 {
                continue;
            }

            let raw_box = NormalizedBox {
                x: min_x as f32 / w as f32,
                y: min_y as f32 / h as f32,
                w: bw / w as f32,
                h: bh / h as f32,
            };
            // DBNet shrinks ground-truth text boxes during training; expand slightly to restore full glyphs.
            boxes.push((raw_box.expand(0.22), mean_score));
        }
    }
    boxes
}

/// Checks whether OCR-recognized screen text contains sensitive PII (email, phone/CCCD/bank digits,
/// API keys/secrets, or authenticated URLs).
pub fn is_screen_pii_text(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.len() < 5 {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    for secret_prefix in ["sk-", "pk_live_", "ghp_", "gho_", "aiza", "akia", "xoxb-", "bearer "] {
        if lower.contains(secret_prefix) {
            return true;
        }
    }

    // Email address pattern: local@domain.tld
    for token in trimmed.split_whitespace() {
        let tok = token.trim_matches(|c: char| matches!(c, ',' | '.' | ';' | ':' | ')' | '(' | '<' | '>'));
        if let Some((local, domain)) = tok.split_once('@')
            && !local.is_empty()
            && domain.contains('.')
            && domain.len() >= 4
        {
            return true;
        }
    }

    // Long digit runs (phones, bank accounts, credit cards, Vietnamese 12-digit CCCD).
    let mut digits_total = 0usize;
    let mut max_run = 0usize;
    let mut cur_run = 0usize;
    for ch in trimmed.chars() {
        if ch.is_ascii_digit() {
            digits_total += 1;
            cur_run += 1;
            max_run = max_run.max(cur_run);
        } else if matches!(ch, '-' | ' ' | '.' | '+' | '(' | ')') {
            // Separators allowed within a phone/card number.
        } else {
            cur_run = 0;
        }
    }
    if digits_total >= 9 && max_run >= 8 {
        return true;
    }

    false
}
