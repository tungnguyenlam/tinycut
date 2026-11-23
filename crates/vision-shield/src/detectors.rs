use std::path::Path;
use tinycut_core_domain::{DomainResult, NormalizedBox, RedactionKind, ShieldConfig};
use tinycut_media_engine::DecodedFrame;

#[cfg(feature = "onnx")]
use tinycut_core_domain::DomainError;

use crate::FrameDetection;
#[cfg(feature = "qr")]
use crate::image_ops::rgba_to_luma8;
#[cfg(feature = "onnx")]
use crate::image_ops::{
    crop_rgba, extract_text_boxes_from_prob_map, is_screen_pii_text, l2_normalize, nms_detections,
    resize_rgba_bilinear, rgba_to_nchw_bgr_255, rgba_to_nchw_rgb_01, rgba_to_nchw_rgb_imagenet,
    rgba_to_nchw_rgb_neg1_1,
};

/// PaddleOCR standard 95-character English/ASCII dictionary (`en_dict.txt`, CTC index 1..=95; 0 = blank).
#[cfg(feature = "onnx")]
const PPOCR_EN_DICT: &[&str] = &[
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", ":", ";", "<", "=", ">", "?", "@", "A", "B",
    "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U",
    "V", "W", "X", "Y", "Z", "[", "\\", "]", "^", "_", "`", "a", "b", "c", "d", "e", "f", "g", "h",
    "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z", "{",
    "|", "}", "~", "!", "\"", "#", "$", "%", "&", "'", "(", ")", "*", "+", ",", "-", ".", "/", " ",
];

/// Pure-Rust QR code detector using `rqrr` (built-in, zero external model weights required).
#[cfg(feature = "qr")]
pub fn detect_qr_codes(frame: &DecodedFrame) -> Vec<FrameDetection> {
    if frame.width == 0 || frame.height == 0 {
        return Vec::new();
    }
    let luma = rgba_to_luma8(&frame.rgba, frame.width, frame.height);
    let w = frame.width as usize;
    let h = frame.height as usize;

    let mut prepared = rqrr::PreparedImage::prepare_from_greyscale(w, h, |x, y| luma[y * w + x]);
    let grids = prepared.detect_grids();
    let mut dets = Vec::with_capacity(grids.len());

    for grid in grids {
        let pts = grid.bounds;
        let min_x = pts.iter().map(|p| p.x).min().unwrap_or(0).max(0) as f32;
        let max_x = pts.iter().map(|p| p.x).max().unwrap_or(0).min(w as i32) as f32;
        let min_y = pts.iter().map(|p| p.y).min().unwrap_or(0).max(0) as f32;
        let max_y = pts.iter().map(|p| p.y).max().unwrap_or(0).min(h as i32) as f32;
        let bw = (max_x - min_x) / frame.width as f32;
        let bh = (max_y - min_y) / frame.height as f32;
        if bw > 0.005 && bh > 0.005 {
            dets.push(FrameDetection {
                frame_index: frame.frame_index,
                kind: RedactionKind::QrCode,
                bbox: NormalizedBox {
                    x: (min_x / frame.width as f32).clamp(0.0, 1.0),
                    y: (min_y / frame.height as f32).clamp(0.0, 1.0),
                    w: bw.clamp(0.0, 1.0),
                    h: bh.clamp(0.0, 1.0),
                },
                confidence: 0.98,
                embedding: None,
            });
        }
    }
    dets
}

#[cfg(not(feature = "qr"))]
pub fn detect_qr_codes(_frame: &DecodedFrame) -> Vec<FrameDetection> {
    Vec::new()
}

/// Reusable set of loaded ONNX Runtime sessions for a single Privacy Shield scan pass.
#[cfg(feature = "onnx")]
pub struct OnnxDetectorBundle {
    yunet: Option<ort::session::Session>,
    sface: Option<ort::session::Session>,
    plate: Option<ort::session::Session>,
    ocr_det: Option<ort::session::Session>,
    ocr_rec: Option<ort::session::Session>,
    face_conf_threshold: f32,
    plate_conf_threshold: f32,
}

#[cfg(feature = "onnx")]
impl OnnxDetectorBundle {
    pub fn load(engine: &crate::LocalVisionEngine, cfg: &ShieldConfig) -> DomainResult<Self> {
        let yunet = if cfg.detect_faces {
            load_optional_session(engine.face_detector_onnx.as_deref(), "YuNet face detector")?
        } else {
            None
        };
        let sface = if cfg.detect_faces && yunet.is_some() {
            load_optional_session(engine.face_embedder_onnx.as_deref(), "SFace face embedder")?
        } else {
            None
        };
        let plate = if cfg.detect_plates {
            load_optional_session(
                engine.plate_detector_onnx.as_deref(),
                "RT-DETRv2 plate detector",
            )?
        } else {
            None
        };
        let ocr_det = if cfg.detect_screen_pii {
            load_optional_session(
                engine.screen_ocr_det_onnx.as_deref(),
                "PP-OCRv4 text detector",
            )?
        } else {
            None
        };
        let ocr_rec = if cfg.detect_screen_pii && ocr_det.is_some() {
            load_optional_session(
                engine.screen_ocr_rec_onnx.as_deref(),
                "PP-OCRv3 text recognizer",
            )?
        } else {
            None
        };

        Ok(Self {
            yunet,
            sface,
            plate,
            ocr_det,
            ocr_rec,
            face_conf_threshold: engine.face_conf_threshold,
            plate_conf_threshold: engine.plate_conf_threshold,
        })
    }

    pub fn detect_frame(
        &mut self,
        frame: &DecodedFrame,
        cfg: &ShieldConfig,
    ) -> DomainResult<Vec<FrameDetection>> {
        let mut all = Vec::new();

        if cfg.detect_faces && self.yunet.is_some() {
            let mut faces = self.detect_faces_yunet(frame)?;
            if self.sface.is_some() {
                for det in &mut faces {
                    if let Ok(emb) = self.embed_face_sface(frame, det.bbox) {
                        det.embedding = Some(emb);
                    }
                }
            }
            all.extend(faces);
        }

        if cfg.detect_plates && self.plate.is_some() {
            all.extend(self.detect_plates_rtdetr(frame)?);
        }

        if cfg.detect_screen_pii && self.ocr_det.is_some() && self.ocr_rec.is_some() {
            all.extend(self.detect_screen_pii_ppocr(frame)?);
        }

        if cfg.detect_qr {
            all.extend(detect_qr_codes(frame));
        }

        Ok(all)
    }

    /// Runs **YuNet 2023mar** (`face_detection_yunet_2023mar.onnx`) using OpenCV's exact
    /// stride 8/16/32 decode formula at its fixed `640x640` ONNX input size.
    fn detect_faces_yunet(&mut self, frame: &DecodedFrame) -> DomainResult<Vec<FrameDetection>> {
        use ort::value::Tensor;

        const INPUT_SIZE: u32 = 640;
        let resized = resize_rgba_bilinear(
            &frame.rgba,
            frame.width,
            frame.height,
            INPUT_SIZE,
            INPUT_SIZE,
        );
        let bgr = rgba_to_nchw_bgr_255(&resized, INPUT_SIZE, INPUT_SIZE);
        let input = Tensor::from_array(([1_usize, 3, INPUT_SIZE as usize, INPUT_SIZE as usize], bgr))
            .map_err(|e| DomainError::Inference(format!("yunet tensor error: {e}")))?;

        let session = self
            .yunet
            .as_mut()
            .ok_or_else(|| DomainError::Inference("yunet session missing".into()))?;
        let input_name = session.inputs()[0].name().to_string();
        let outputs = session
            .run(ort::inputs![input_name.as_str() => input])
            .map_err(|e| DomainError::Inference(format!("yunet inference failed: {e}")))?;

        let mut raw_dets = Vec::new();
        for stride in [8_u32, 16, 32] {
            let cols = (INPUT_SIZE / stride) as usize;
            let rows = (INPUT_SIZE / stride) as usize;
            let cls_name = format!("cls_{stride}");
            let obj_name = format!("obj_{stride}");
            let bbox_name = format!("bbox_{stride}");

            let (_, cls_v) = outputs[cls_name.as_str()]
                .try_extract_tensor::<f32>()
                .map_err(|e| DomainError::Inference(format!("yunet {cls_name} extract: {e}")))?;
            let (_, obj_v) = outputs[obj_name.as_str()]
                .try_extract_tensor::<f32>()
                .map_err(|e| DomainError::Inference(format!("yunet {obj_name} extract: {e}")))?;
            let (_, bbox_v) = outputs[bbox_name.as_str()]
                .try_extract_tensor::<f32>()
                .map_err(|e| DomainError::Inference(format!("yunet {bbox_name} extract: {e}")))?;

            for r in 0..rows {
                for c in 0..cols {
                    let idx = r * cols + c;
                    let cls_score = cls_v[idx].clamp(0.0, 1.0);
                    let obj_score = obj_v[idx].clamp(0.0, 1.0);
                    let score = (cls_score * obj_score).sqrt();
                    if score < self.face_conf_threshold {
                        continue;
                    }

                    let b = idx * 4;
                    let cx = (c as f32 + bbox_v[b]) * stride as f32;
                    let cy = (r as f32 + bbox_v[b + 1]) * stride as f32;
                    let bw = bbox_v[b + 2].exp() * stride as f32;
                    let bh = bbox_v[b + 3].exp() * stride as f32;

                    let x1 = ((cx - bw * 0.5) / INPUT_SIZE as f32).clamp(0.0, 1.0);
                    let y1 = ((cy - bh * 0.5) / INPUT_SIZE as f32).clamp(0.0, 1.0);
                    let x2 = ((cx + bw * 0.5) / INPUT_SIZE as f32).clamp(0.0, 1.0);
                    let y2 = ((cy + bh * 0.5) / INPUT_SIZE as f32).clamp(0.0, 1.0);

                    if x2 > x1 && y2 > y1 {
                        raw_dets.push(FrameDetection {
                            frame_index: frame.frame_index,
                            kind: RedactionKind::Face,
                            bbox: NormalizedBox {
                                x: x1,
                                y: y1,
                                w: x2 - x1,
                                h: y2 - y1,
                            },
                            confidence: score,
                            embedding: None,
                        });
                    }
                }
            }
        }

        Ok(nms_detections(raw_dets, 0.30))
    }

    /// Runs **SFace 2021dec** (`face_recognition_sface_2021dec.onnx`) on a cropped face box to
    /// produce an L2-normalized 128-dim identity embedding for Creator Face Whitelist matching.
    pub fn embed_face_sface(
        &mut self,
        frame: &DecodedFrame,
        bbox: NormalizedBox,
    ) -> DomainResult<Vec<f32>> {
        use ort::value::Tensor;

        let (cw, ch, crop) = crop_rgba(&frame.rgba, frame.width, frame.height, bbox.expand(0.15));
        if cw == 0 || ch == 0 {
            return Err(DomainError::Inference("empty face crop".into()));
        }
        let resized = resize_rgba_bilinear(&crop, cw, ch, 112, 112);
        let rgb = rgba_to_nchw_rgb_neg1_1(&resized, 112, 112);
        let input = Tensor::from_array(([1_usize, 3, 112, 112], rgb))
            .map_err(|e| DomainError::Inference(format!("sface tensor error: {e}")))?;

        let session = self
            .sface
            .as_mut()
            .ok_or_else(|| DomainError::Inference("sface session missing".into()))?;
        let input_name = session.inputs()[0].name().to_string();
        let outputs = session
            .run(ort::inputs![input_name.as_str() => input])
            .map_err(|e| DomainError::Inference(format!("sface inference failed: {e}")))?;

        let (_, feat) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| DomainError::Inference(format!("sface output extract: {e}")))?;
        let mut embedding = feat.to_vec();
        l2_normalize(&mut embedding);
        Ok(embedding)
    }

    /// Runs **RT-DETRv2 R18 Plate** (`plate_rtdetr.onnx`, Apache-2.0):
    /// - Input: `pixel_values` `[1, 3, 640, 640]` RGB in `[0, 1]`
    /// - Outputs: `logits` `[1, 300, 1]` (sigmoid), `pred_boxes` `[1, 300, 4]` normalized `(cx, cy, w, h)`
    fn detect_plates_rtdetr(&mut self, frame: &DecodedFrame) -> DomainResult<Vec<FrameDetection>> {
        use ort::value::Tensor;

        const INPUT_SIZE: u32 = 640;
        let resized = resize_rgba_bilinear(
            &frame.rgba,
            frame.width,
            frame.height,
            INPUT_SIZE,
            INPUT_SIZE,
        );
        let rgb = rgba_to_nchw_rgb_01(&resized, INPUT_SIZE, INPUT_SIZE);
        let input = Tensor::from_array(([1_usize, 3, INPUT_SIZE as usize, INPUT_SIZE as usize], rgb))
            .map_err(|e| DomainError::Inference(format!("rtdetr tensor error: {e}")))?;

        let session = self
            .plate
            .as_mut()
            .ok_or_else(|| DomainError::Inference("plate detector session missing".into()))?;
        let input_name = session.inputs()[0].name().to_string();
        let outputs = session
            .run(ort::inputs![input_name.as_str() => input])
            .map_err(|e| DomainError::Inference(format!("plate rtdetr inference failed: {e}")))?;

        let (_, logits) = outputs["logits"]
            .try_extract_tensor::<f32>()
            .map_err(|e| DomainError::Inference(format!("rtdetr logits extract: {e}")))?;
        let (_, boxes) = outputs["pred_boxes"]
            .try_extract_tensor::<f32>()
            .map_err(|e| DomainError::Inference(format!("rtdetr pred_boxes extract: {e}")))?;

        let num_queries = logits.len().min(boxes.len() / 4);
        let mut raw_dets = Vec::new();
        for (i, &logit) in logits.iter().enumerate().take(num_queries) {
            let score = 1.0 / (1.0 + (-logit).exp());
            if score < self.plate_conf_threshold {
                continue;
            }
            let b = i * 4;
            let cx = boxes[b];
            let cy = boxes[b + 1];
            let bw = boxes[b + 2].clamp(0.0, 1.0);
            let bh = boxes[b + 3].clamp(0.0, 1.0);
            let x1 = (cx - bw * 0.5).clamp(0.0, 1.0);
            let y1 = (cy - bh * 0.5).clamp(0.0, 1.0);
            let x2 = (cx + bw * 0.5).clamp(0.0, 1.0);
            let y2 = (cy + bh * 0.5).clamp(0.0, 1.0);

            if x2 > x1 && y2 > y1 {
                raw_dets.push(FrameDetection {
                    frame_index: frame.frame_index,
                    kind: RedactionKind::LicensePlate,
                    bbox: NormalizedBox {
                        x: x1,
                        y: y1,
                        w: x2 - x1,
                        h: y2 - y1,
                    },
                    confidence: score,
                    embedding: None,
                });
            }
        }

        Ok(nms_detections(raw_dets, 0.45))
    }

    /// Runs **PP-OCRv4 Mobile Det + PP-OCRv3 Mobile En Rec** followed by on-device PII matching
    /// (emails, phones/IDs, API keys) to detect `RedactionKind::ScreenPii` regions.
    fn detect_screen_pii_ppocr(
        &mut self,
        frame: &DecodedFrame,
    ) -> DomainResult<Vec<FrameDetection>> {
        use ort::value::Tensor;

        // Resize longest side <= 640 with dimensions rounded to multiples of 32.
        let scale = (640.0 / frame.width.max(frame.height).max(1) as f32).min(1.0);
        let det_w = (((frame.width as f32 * scale) / 32.0).round().max(1.0) as u32) * 32;
        let det_h = (((frame.height as f32 * scale) / 32.0).round().max(1.0) as u32) * 32;

        let resized = resize_rgba_bilinear(&frame.rgba, frame.width, frame.height, det_w, det_h);
        let det_in = rgba_to_nchw_rgb_imagenet(&resized, det_w, det_h);
        let input = Tensor::from_array(([1_usize, 3, det_h as usize, det_w as usize], det_in))
            .map_err(|e| DomainError::Inference(format!("ppocr det tensor error: {e}")))?;

        let candidate_boxes = {
            let det_session = self
                .ocr_det
                .as_mut()
                .ok_or_else(|| DomainError::Inference("ppocr det session missing".into()))?;
            let det_in_name = det_session.inputs()[0].name().to_string();
            let det_out = det_session
                .run(ort::inputs![det_in_name.as_str() => input])
                .map_err(|e| DomainError::Inference(format!("ppocr det inference failed: {e}")))?;

            let (_, prob_map) = det_out[0]
                .try_extract_tensor::<f32>()
                .map_err(|e| DomainError::Inference(format!("ppocr det output extract: {e}")))?;

            extract_text_boxes_from_prob_map(prob_map, det_w as usize, det_h as usize, 0.30, 0.50)
        };
        let mut pii_dets = Vec::new();

        for (bbox, det_score) in candidate_boxes.into_iter().take(40) {
            let text = self.recognize_text_crop_ppocr(frame, bbox)?;
            if is_screen_pii_text(&text) {
                pii_dets.push(FrameDetection {
                    frame_index: frame.frame_index,
                    kind: RedactionKind::ScreenPii,
                    bbox,
                    confidence: det_score.clamp(0.5, 0.99),
                    embedding: None,
                });
            }
        }

        Ok(nms_detections(pii_dets, 0.40))
    }

    pub(crate) fn recognize_text_crop_ppocr(
        &mut self,
        frame: &DecodedFrame,
        bbox: NormalizedBox,
    ) -> DomainResult<String> {
        use ort::value::Tensor;

        let (cw, ch, crop) = crop_rgba(&frame.rgba, frame.width, frame.height, bbox);
        if cw == 0 || ch == 0 {
            return Ok(String::new());
        }

        const REC_H: u32 = 48;
        let rec_w = (((cw as f32 / ch as f32) * REC_H as f32).round() as u32).clamp(32, 320);
        let resized = resize_rgba_bilinear(&crop, cw, ch, rec_w, REC_H);
        let rec_in = rgba_to_nchw_rgb_neg1_1(&resized, rec_w, REC_H);
        let input = Tensor::from_array(([1_usize, 3, REC_H as usize, rec_w as usize], rec_in))
            .map_err(|e| DomainError::Inference(format!("ppocr rec tensor error: {e}")))?;

        let rec_session = self
            .ocr_rec
            .as_mut()
            .ok_or_else(|| DomainError::Inference("ppocr rec session missing".into()))?;
        let rec_in_name = rec_session.inputs()[0].name().to_string();
        let rec_out = rec_session
            .run(ort::inputs![rec_in_name.as_str() => input])
            .map_err(|e| DomainError::Inference(format!("ppocr rec inference failed: {e}")))?;

        let (shape, probs) = rec_out[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| DomainError::Inference(format!("ppocr rec output extract: {e}")))?;

        if shape.len() != 3 {
            return Ok(String::new());
        }
        let steps = shape[1].max(0) as usize;
        let classes = shape[2].max(0) as usize;
        Ok(ctc_greedy_decode_en(probs, steps, classes))
    }
}

#[cfg(feature = "onnx")]
fn ctc_greedy_decode_en(probs: &[f32], steps: usize, classes: usize) -> String {
    if steps == 0 || classes == 0 || probs.len() < steps * classes {
        return String::new();
    }
    let mut out = String::new();
    let mut prev_idx: Option<usize> = None;

    for t in 0..steps {
        let row = &probs[t * classes..(t + 1) * classes];
        let (best_idx, _) = row
            .iter()
            .enumerate()
            .fold((0usize, f32::NEG_INFINITY), |(bi, bv), (i, &v)| {
                if v > bv { (i, v) } else { (bi, bv) }
            });

        // Index 0 is CTC blank in PaddleOCR.
        if best_idx != 0
            && prev_idx != Some(best_idx)
            && let Some(ch) = PPOCR_EN_DICT.get(best_idx - 1)
        {
            out.push_str(ch);
        }
        prev_idx = Some(best_idx);
    }
    out
}

#[cfg(feature = "onnx")]
fn ort_environment() -> DomainResult<&'static ort::environment::Environment> {
    static ENV: std::sync::OnceLock<ort::environment::Environment> = std::sync::OnceLock::new();
    if let Some(env) = ENV.get() {
        return Ok(env);
    }
    let created = ort::init()
        .with_name("tinycut-vision")
        .build()
        .map_err(|e| DomainError::Inference(format!("ort environment init failed: {e}")))?;
    let _ = ENV.set(created);
    ENV.get()
        .ok_or_else(|| DomainError::Inference("ort environment unavailable".into()))
}

#[cfg(feature = "onnx")]
fn load_optional_session(
    path: Option<&Path>,
    label: &str,
) -> DomainResult<Option<ort::session::Session>> {
    let Some(p) = path else {
        return Ok(None);
    };
    if !p.exists() {
        return Ok(None);
    }
    let env = ort_environment()?;
    let session = ort::session::Session::builder(env)
        .map_err(|e| DomainError::Inference(format!("{label} session builder error: {e}")))?
        .with_intra_threads(4)
        .map_err(|e| DomainError::Inference(format!("{label} threads error: {e}")))?
        .commit_from_file(p)
        .map_err(|e| {
            DomainError::Inference(format!(
                "failed to load {label} ONNX from {}: {e}",
                p.display()
            ))
        })?;
    Ok(Some(session))
}

/// Runs standalone QR-only detection when ONNX feature is disabled.
#[cfg(not(feature = "onnx"))]
pub fn detect_frame_non_onnx(
    frame: &DecodedFrame,
    cfg: &ShieldConfig,
) -> DomainResult<Vec<FrameDetection>> {
    if cfg.detect_qr {
        Ok(detect_qr_codes(frame))
    } else {
        Ok(Vec::new())
    }
}
