use crate::bridge::{apply_theme_to_document, detect_initial_is_light};
use dioxus::prelude::*;
use std::collections::{HashMap, HashSet};
use tinycut_core_domain::{
    GlossaryEntry, InferenceMode, MediaRef, ModelInstallStatus, NormalizedBox, Project,
    RedactionKeyframe, RedactionKind, RedactionTrack, ShieldConfig, Transcript, TranscriptSegment,
    default_lightweight_models,
};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StudioMode {
    Subtitles,
    Shield,
    Glossary,
    Media,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StudioTheme {
    Light,
    Dark,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrackVerdict {
    NotPii,
    Whitelisted,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToastSeverity {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, PartialEq, Debug)]
pub struct ToastItem {
    pub id: u64,
    pub severity: ToastSeverity,
    pub message: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExportFormat {
    Srt,
    Ass,
    Fcpxml,
    PremiereXml,
    Edl,
}

impl ExportFormat {
    pub const ALL: [ExportFormat; 5] = [
        Self::Srt,
        Self::Ass,
        Self::Fcpxml,
        Self::PremiereXml,
        Self::Edl,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Srt => "srt",
            Self::Ass => "ass",
            Self::Fcpxml => "fcpxml",
            Self::PremiereXml => "premiere_xml",
            Self::Edl => "edl",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Srt => "SRT",
            Self::Ass => "ASS",
            Self::Fcpxml => "FCPXML",
            Self::PremiereXml => "Premiere XML",
            Self::Edl => "DaVinci EDL",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Srt => "YouTube, CapCut, universal",
            Self::Ass => "Styled bilingual burn-in",
            Self::Fcpxml => "Final Cut Pro 10.6+ titles",
            Self::PremiereXml => "XMEML v5 timeline markers",
            Self::Edl => "CMX 3600 Resolve / Pro",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Srt => "srt",
            Self::Ass => "ass",
            Self::Fcpxml => "fcpxml",
            Self::PremiereXml => "xml",
            Self::Edl => "edl",
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct UndoSnapshot {
    pub transcript: Transcript,
    pub tracks: Vec<RedactionTrack>,
    pub track_verdicts: HashMap<u64, TrackVerdict>,
    pub beeped_segments: HashSet<Uuid>,
}

#[derive(Clone, Copy)]
pub struct StudioState {
    // Active Project & Media
    pub project: Signal<Project>,
    pub recent_projects: Signal<Vec<Project>>,
    pub is_demo_media: Signal<bool>,
    pub dirty: Signal<bool>,

    // Playback & Selection
    pub current_ms: Signal<u64>,
    pub playing: Signal<bool>,
    pub playback_rate: Signal<f32>,
    pub selected_segment_id: Signal<Option<Uuid>>,
    pub selected_track_id: Signal<Option<u64>>,

    // Workspace Mode & UI Panels
    pub mode: Signal<StudioMode>,
    pub theme: Signal<StudioTheme>,
    pub show_export_drawer: Signal<bool>,
    pub show_project_menu: Signal<bool>,
    pub show_privacy_menu: Signal<bool>,
    pub show_view_menu: Signal<bool>,
    pub show_shortcuts_modal: Signal<bool>,
    pub editing_text: Signal<bool>,

    // Subtitles & Glossary
    pub transcript: Signal<Transcript>,
    pub show_diffs: Signal<bool>,
    pub show_subtitles_overlay: Signal<bool>,
    pub sub_filter: Signal<String>,
    pub sub_only_pii: Signal<bool>,
    pub glossary: Signal<Vec<GlossaryEntry>>,
    pub beeped_segments: Signal<HashSet<Uuid>>,

    // Timeline View
    pub timeline_zoom: Signal<f64>,
    pub timeline_snap: Signal<bool>,

    // Privacy Shield
    pub shield_cfg: Signal<ShieldConfig>,
    pub tracks: Signal<Vec<RedactionTrack>>,
    pub track_verdicts: Signal<HashMap<u64, TrackVerdict>>,
    pub show_masks: Signal<bool>,
    pub show_zero_leak_margin: Signal<bool>,
    pub opt_in_telemetry: Signal<bool>,

    // Export Drawer
    pub export_format: Signal<ExportFormat>,
    pub export_strip_fillers: Signal<bool>,
    pub export_include_speaker: Signal<bool>,
    pub export_max_chars: Signal<usize>,
    pub export_copied: Signal<bool>,

    // Local Lightweight Models
    pub models: Signal<Vec<ModelInstallStatus>>,

    // Status, Jobs & Toasts
    pub busy_job: Signal<Option<String>>,
    pub status_msg: Signal<(ToastSeverity, String)>,
    pub toasts: Signal<Vec<ToastItem>>,
    pub next_toast_id: Signal<u64>,

    // Undo / Redo History
    pub undo_stack: Signal<Vec<UndoSnapshot>>,
    pub redo_stack: Signal<Vec<UndoSnapshot>>,
}

impl StudioState {
    pub fn new() -> Self {
        let demo_project = sample_project();
        let demo_transcript = sample_transcript();
        let first_seg = demo_transcript.segments.first().map(|s| s.id);
        let mut verdicts = HashMap::new();
        verdicts.insert(2, TrackVerdict::Whitelisted);

        let initial_theme = if detect_initial_is_light() {
            StudioTheme::Light
        } else {
            StudioTheme::Dark
        };
        apply_theme_to_document(initial_theme == StudioTheme::Light, false);

        Self {
            project: Signal::new(demo_project.clone()),
            recent_projects: Signal::new(vec![demo_project]),
            is_demo_media: Signal::new(true),
            dirty: Signal::new(false),

            current_ms: Signal::new(1_200),
            playing: Signal::new(false),
            playback_rate: Signal::new(1.0),
            selected_segment_id: Signal::new(first_seg),
            selected_track_id: Signal::new(Some(1)),

            mode: Signal::new(StudioMode::Subtitles),
            theme: Signal::new(initial_theme),
            show_export_drawer: Signal::new(false),
            show_project_menu: Signal::new(false),
            show_privacy_menu: Signal::new(false),
            show_view_menu: Signal::new(false),
            show_shortcuts_modal: Signal::new(false),
            editing_text: Signal::new(false),

            transcript: Signal::new(demo_transcript),
            show_diffs: Signal::new(true),
            show_subtitles_overlay: Signal::new(true),
            sub_filter: Signal::new(String::new()),
            sub_only_pii: Signal::new(false),
            glossary: Signal::new(sample_glossary()),
            beeped_segments: Signal::new(HashSet::new()),

            timeline_zoom: Signal::new(76.0),
            timeline_snap: Signal::new(true),

            shield_cfg: Signal::new(ShieldConfig {
                detect_plates: true,
                detect_faces: true,
                detect_screen_pii: true,
                detect_qr: true,
                ..ShieldConfig::default()
            }),
            tracks: Signal::new(sample_tracks()),
            track_verdicts: Signal::new(verdicts),
            show_masks: Signal::new(true),
            show_zero_leak_margin: Signal::new(true),
            opt_in_telemetry: Signal::new(false),

            export_format: Signal::new(ExportFormat::Srt),
            export_strip_fillers: Signal::new(true),
            export_include_speaker: Signal::new(false),
            export_max_chars: Signal::new(42),
            export_copied: Signal::new(false),

            models: Signal::new(
                default_lightweight_models()
                    .into_iter()
                    .map(|spec| ModelInstallStatus {
                        spec,
                        installed: false,
                        local_path: None,
                        bytes_on_disk: 0,
                    })
                    .collect(),
            ),

            busy_job: Signal::new(None),
            status_msg: Signal::new((
                ToastSeverity::Success,
                "Ready — 100% local & offline (Street Vlog 4K Demo)".into(),
            )),
            toasts: Signal::new(Vec::new()),
            next_toast_id: Signal::new(1),

            undo_stack: Signal::new(Vec::new()),
            redo_stack: Signal::new(Vec::new()),
        }
    }

    pub fn snapshot(&self) -> UndoSnapshot {
        UndoSnapshot {
            transcript: self.transcript.read().clone(),
            tracks: self.tracks.read().clone(),
            track_verdicts: self.track_verdicts.read().clone(),
            beeped_segments: self.beeped_segments.read().clone(),
        }
    }

    pub fn push_undo(&mut self) {
        let snap = self.snapshot();
        let mut stack = self.undo_stack.write();
        stack.push(snap);
        if stack.len() > 50 {
            stack.remove(0);
        }
        self.redo_stack.write().clear();
        self.dirty.set(true);
    }

    pub fn undo(&mut self) {
        let prev = self.undo_stack.write().pop();
        if let Some(snap) = prev {
            let current = self.snapshot();
            self.redo_stack.write().push(current);
            self.transcript.set(snap.transcript);
            self.tracks.set(snap.tracks);
            self.track_verdicts.set(snap.track_verdicts);
            self.beeped_segments.set(snap.beeped_segments);
            self.notify(ToastSeverity::Info, "Undid last edit");
        }
    }

    pub fn redo(&mut self) {
        let next = self.redo_stack.write().pop();
        if let Some(snap) = next {
            let current = self.snapshot();
            self.undo_stack.write().push(current);
            self.transcript.set(snap.transcript);
            self.tracks.set(snap.tracks);
            self.track_verdicts.set(snap.track_verdicts);
            self.beeped_segments.set(snap.beeped_segments);
            self.notify(ToastSeverity::Info, "Redid edit");
        }
    }

    pub fn notify(&mut self, severity: ToastSeverity, msg: impl Into<String>) {
        let message = msg.into();
        self.status_msg.set((severity, message.clone()));
        let id = {
            let mut next = self.next_toast_id.write();
            let current = *next;
            *next += 1;
            current
        };
        let mut list = self.toasts.write();
        list.push(ToastItem {
            id,
            severity,
            message,
        });
        if list.len() > 3 {
            list.remove(0);
        }
    }

    pub fn dismiss_toast(&mut self, id: u64) {
        self.toasts.write().retain(|t| t.id != id);
    }

    pub fn seek_ms(&mut self, ms: u64) {
        let dur = self.project.read().media.duration_ms.max(1);
        self.current_ms.set(ms.min(dur));
    }

    pub fn toggle_play(&mut self) {
        let now = *self.playing.read();
        if !now {
            let cur = *self.current_ms.read();
            let dur = self.project.read().media.duration_ms;
            if cur >= dur.saturating_sub(50) {
                self.current_ms.set(0);
            }
        }
        self.playing.set(!now);
    }

    pub fn toggle_theme(&mut self) {
        let next = match *self.theme.read() {
            StudioTheme::Light => StudioTheme::Dark,
            StudioTheme::Dark => StudioTheme::Light,
        };
        self.theme.set(next);
        apply_theme_to_document(next == StudioTheme::Light, true);
    }

    pub fn close_overlays(&mut self) {
        self.show_export_drawer.set(false);
        self.show_project_menu.set(false);
        self.show_privacy_menu.set(false);
        self.show_view_menu.set(false);
        self.show_shortcuts_modal.set(false);
    }

    pub fn load_demo(&mut self) {
        let proj = sample_project();
        let tr = sample_transcript();
        let first_seg = tr.segments.first().map(|s| s.id);
        let mut verdicts = HashMap::new();
        verdicts.insert(2, TrackVerdict::Whitelisted);

        self.project.set(proj);
        self.is_demo_media.set(true);
        self.transcript.set(tr);
        self.tracks.set(sample_tracks());
        self.track_verdicts.set(verdicts);
        self.beeped_segments.write().clear();
        self.selected_segment_id.set(first_seg);
        self.selected_track_id.set(Some(1));
        self.current_ms.set(1_200);
        self.playing.set(false);
        self.dirty.set(false);
        self.notify(
            ToastSeverity::Info,
            "Loaded interactive 4K Street Vlog demo project",
        );
    }
}

// ============================================================================
// Rich Bilingual Street-Vlog Demo Data
// ============================================================================

pub fn sample_media(path: &str) -> MediaRef {
    MediaRef {
        id: Uuid::new_v4(),
        path: path.to_string(),
        duration_ms: 14_000,
        width: 3840,
        height: 2160,
        fps: 60.0,
    }
}

pub fn sample_project() -> Project {
    Project {
        id: Uuid::new_v4(),
        title: "saigon_street_vlog_4k".into(),
        media: sample_media("/videos/saigon_street_vlog_4k.mp4"),
        is_nda_private: true,
        inference_mode: InferenceMode::Local,
        locale: "vi-VN".into(),
        content_domain: Some("street_vlog".into()),
    }
}

pub fn sample_glossary() -> Vec<GlossaryEntry> {
    vec![
        GlossaryEntry {
            spoken_variant: "cơ sơ".into(),
            preferred_term: "Cursor".into(),
            hit_count: 6,
        },
        GlossaryEntry {
            spoken_variant: "mác búc".into(),
            preferred_term: "MacBook Air M4".into(),
            hit_count: 4,
        },
        GlossaryEntry {
            spoken_variant: "đà vĩnh xi".into(),
            preferred_term: "DaVinci Resolve".into(),
            hit_count: 2,
        },
    ]
}

pub fn sample_transcript() -> Transcript {
    Transcript {
        locale: "vi-VN".into(),
        asr_model_version: "whisper-base-q5_1".into(),
        llm_model_version: Some("qwen2.5-0.5b-instruct-q4_k_m".into()),
        segments: vec![
            TranscriptSegment {
                id: Uuid::new_v4(),
                start_ms: 400,
                end_ms: 3_600,
                raw_asr: "ờ, hôm nay mình review cơ sơ trên mác búc ở quận nhất".into(),
                llm_draft: Some("Hôm nay mình review Cursor trên MacBook ở Quận 1.".into()),
                text: "Hôm nay mình review Cursor trên MacBook Air M4 ở Quận 1.".into(),
                speaker: Some("Host".into()),
                words: vec![],
            },
            TranscriptSegment {
                id: Uuid::new_v4(),
                start_ms: 4_100,
                end_ms: 7_400,
                raw_asr: "kiểu như quay vlog ngoài đường thì rất cần tự động che biển số xe".into(),
                llm_draft: Some(
                    "Quay vlog ngoài đường thì rất cần tự động che biển số xe.".into(),
                ),
                text: "kiểu như quay vlog ngoài đường thì rất cần tự động che biển số xe.".into(),
                speaker: Some("Host".into()),
                words: vec![],
            },
            TranscriptSegment {
                id: Uuid::new_v4(),
                start_ms: 8_100,
                end_ms: 10_900,
                raw_asr: "à, giữ nguyên mặt mình còn người đi đường thì làm mờ bằng tai ni cắt".into(),
                llm_draft: Some(
                    "Giữ nguyên mặt mình còn người đi đường thì làm mờ bằng TinyCut.".into(),
                ),
                text: "Giữ nguyên mặt mình còn người đi đường thì làm mờ bằng TinyCut.".into(),
                speaker: Some("Host".into()),
                words: vec![],
            },
            TranscriptSegment {
                id: Uuid::new_v4(),
                start_ms: 11_300,
                end_ms: 13_600,
                raw_asr: "nhắn qua số 0901 234 567 để mình gửi bản đà vĩnh xi nha".into(),
                llm_draft: Some(
                    "Nhắn qua số 0901 234 567 để mình gửi bản DaVinci Resolve nha.".into(),
                ),
                text: "Nhắn qua số 0901 234 567 để mình gửi bản DaVinci Resolve nha.".into(),
                speaker: Some("Host".into()),
                words: vec![],
            },
        ],
    }
}

/// 60fps * 14s = 840 frames
pub fn sample_tracks() -> Vec<RedactionTrack> {
    vec![
        RedactionTrack {
            track_id: 1,
            kind: RedactionKind::LicensePlate,
            enabled: true,
            whitelisted: false,
            keyframes: vec![
                RedactionKeyframe {
                    frame_index: 20,
                    bbox: NormalizedBox {
                        x: 0.34,
                        y: 0.67,
                        w: 0.15,
                        h: 0.09,
                    },
                    confidence: 0.96,
                },
                RedactionKeyframe {
                    frame_index: 320,
                    bbox: NormalizedBox {
                        x: 0.44,
                        y: 0.65,
                        w: 0.16,
                        h: 0.09,
                    },
                    confidence: 0.94,
                },
                RedactionKeyframe {
                    frame_index: 640,
                    bbox: NormalizedBox {
                        x: 0.56,
                        y: 0.68,
                        w: 0.15,
                        h: 0.09,
                    },
                    confidence: 0.91,
                },
            ],
        },
        RedactionTrack {
            track_id: 2,
            kind: RedactionKind::Face,
            enabled: false,
            whitelisted: true,
            keyframes: vec![
                RedactionKeyframe {
                    frame_index: 0,
                    bbox: NormalizedBox {
                        x: 0.12,
                        y: 0.22,
                        w: 0.13,
                        h: 0.22,
                    },
                    confidence: 0.98,
                },
                RedactionKeyframe {
                    frame_index: 420,
                    bbox: NormalizedBox {
                        x: 0.15,
                        y: 0.20,
                        w: 0.13,
                        h: 0.22,
                    },
                    confidence: 0.97,
                },
                RedactionKeyframe {
                    frame_index: 820,
                    bbox: NormalizedBox {
                        x: 0.13,
                        y: 0.22,
                        w: 0.13,
                        h: 0.22,
                    },
                    confidence: 0.97,
                },
            ],
        },
        RedactionTrack {
            track_id: 3,
            kind: RedactionKind::Face,
            enabled: true,
            whitelisted: false,
            keyframes: vec![
                RedactionKeyframe {
                    frame_index: 90,
                    bbox: NormalizedBox {
                        x: 0.72,
                        y: 0.26,
                        w: 0.11,
                        h: 0.18,
                    },
                    confidence: 0.91,
                },
                RedactionKeyframe {
                    frame_index: 480,
                    bbox: NormalizedBox {
                        x: 0.64,
                        y: 0.25,
                        w: 0.11,
                        h: 0.18,
                    },
                    confidence: 0.88,
                },
                RedactionKeyframe {
                    frame_index: 760,
                    bbox: NormalizedBox {
                        x: 0.70,
                        y: 0.27,
                        w: 0.11,
                        h: 0.18,
                    },
                    confidence: 0.89,
                },
            ],
        },
        RedactionTrack {
            track_id: 4,
            kind: RedactionKind::ScreenPii,
            enabled: true,
            whitelisted: false,
            keyframes: vec![
                RedactionKeyframe {
                    frame_index: 460,
                    bbox: NormalizedBox {
                        x: 0.64,
                        y: 0.08,
                        w: 0.29,
                        h: 0.11,
                    },
                    confidence: 0.93,
                },
                RedactionKeyframe {
                    frame_index: 810,
                    bbox: NormalizedBox {
                        x: 0.64,
                        y: 0.08,
                        w: 0.29,
                        h: 0.11,
                    },
                    confidence: 0.92,
                },
            ],
        },
    ]
}
