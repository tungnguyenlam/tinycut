import { createContext, createSignal, useContext } from "solid-js";
import type {
  GlossaryEntry,
  MediaRef,
  ModelInstallStatus,
  Project,
  RedactionTrack,
  ShieldConfig,
  Transcript,
} from "./bindings";
import { defaultLightweightModels } from "./domainExt";
import {
  applyThemeToDocument,
  commands,
  detectInitialIsLight,
  isTauriRuntime,
} from "./bridge";

export type StudioMode = "subtitles" | "shield" | "glossary" | "media";
export type StudioTheme = "light" | "dark";
export type TrackVerdict = "not_pii" | "whitelisted";
export type ToastSeverity = "info" | "success" | "warning" | "error";

export interface ToastItem {
  id: number;
  severity: ToastSeverity;
  message: string;
}

export type ExportFormat = "srt" | "ass" | "fcpxml" | "premiere_xml" | "edl";

export interface ExportFormatInfo {
  id: ExportFormat;
  label: string;
  hint: string;
  extension: string;
}

export const EXPORT_FORMATS: ExportFormatInfo[] = [
  { id: "srt", label: "SRT", hint: "YouTube, CapCut, universal", extension: "srt" },
  { id: "ass", label: "ASS", hint: "Styled bilingual burn-in", extension: "ass" },
  {
    id: "fcpxml",
    label: "FCPXML",
    hint: "Final Cut Pro 10.6+ titles",
    extension: "fcpxml",
  },
  {
    id: "premiere_xml",
    label: "Premiere XML",
    hint: "XMEML v5 timeline markers",
    extension: "xml",
  },
  { id: "edl", label: "DaVinci EDL", hint: "CMX 3600 Resolve / Pro", extension: "edl" },
];

export function getExportFormatInfo(fmt: ExportFormat): ExportFormatInfo {
  return EXPORT_FORMATS.find((f) => f.id === fmt) ?? EXPORT_FORMATS[0];
}

export interface UndoSnapshot {
  transcript: Transcript;
  tracks: RedactionTrack[];
  trackVerdicts: Record<number, TrackVerdict>;
  beepedSegments: string[];
}

export function newUuid(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === "x" ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}

// ============================================================================
// Rich Bilingual Street-Vlog Demo Data
// ============================================================================

export function sampleMedia(path: string): MediaRef {
  return {
    id: newUuid(),
    path,
    duration_ms: 14_000,
    width: 3840,
    height: 2160,
    fps: 60.0,
  };
}

export function sampleProject(): Project {
  return {
    id: newUuid(),
    title: "saigon_street_vlog_4k",
    media: sampleMedia("/videos/saigon_street_vlog_4k.mp4"),
    is_nda_private: true,
    inference_mode: "local",
    locale: "vi-VN",
    content_domain: "street_vlog",
  };
}

export function sampleGlossary(): GlossaryEntry[] {
  return [
    { spoken_variant: "cơ sơ", preferred_term: "Cursor", hit_count: 6 },
    { spoken_variant: "mác búc", preferred_term: "MacBook Air M4", hit_count: 4 },
    { spoken_variant: "đà vĩnh xi", preferred_term: "DaVinci Resolve", hit_count: 2 },
  ];
}

export function sampleTranscript(): Transcript {
  return {
    locale: "vi-VN",
    asr_model_version: "whisper-base-q5_1",
    llm_model_version: "qwen2.5-0.5b-instruct-q4_k_m",
    segments: [
      {
        id: newUuid(),
        start_ms: 400,
        end_ms: 3_600,
        raw_asr: "ờ, hôm nay mình review cơ sơ trên mác búc ở quận nhất",
        llm_draft: "Hôm nay mình review Cursor trên MacBook ở Quận 1.",
        text: "Hôm nay mình review Cursor trên MacBook Air M4 ở Quận 1.",
        speaker: "Host",
        words: [],
      },
      {
        id: newUuid(),
        start_ms: 4_100,
        end_ms: 7_400,
        raw_asr: "kiểu như quay vlog ngoài đường thì rất cần tự động che biển số xe",
        llm_draft: "Quay vlog ngoài đường thì rất cần tự động che biển số xe.",
        text: "kiểu như quay vlog ngoài đường thì rất cần tự động che biển số xe.",
        speaker: "Host",
        words: [],
      },
      {
        id: newUuid(),
        start_ms: 8_100,
        end_ms: 10_900,
        raw_asr: "à, giữ nguyên mặt mình còn người đi đường thì làm mờ bằng tai ni cắt",
        llm_draft: "Giữ nguyên mặt mình còn người đi đường thì làm mờ bằng TinyCut.",
        text: "Giữ nguyên mặt mình còn người đi đường thì làm mờ bằng TinyCut.",
        speaker: "Host",
        words: [],
      },
      {
        id: newUuid(),
        start_ms: 11_300,
        end_ms: 13_600,
        raw_asr: "nhắn qua số 0901 234 567 để mình gửi bản đà vĩnh xi nha",
        llm_draft: "Nhắn qua số 0901 234 567 để mình gửi bản DaVinci Resolve nha.",
        text: "Nhắn qua số 0901 234 567 để mình gửi bản DaVinci Resolve nha.",
        speaker: "Host",
        words: [],
      },
    ],
  };
}

/** 60fps * 14s = 840 frames */
export function sampleTracks(): RedactionTrack[] {
  return [
    {
      track_id: 1,
      kind: "license_plate",
      enabled: true,
      whitelisted: false,
      keyframes: [
        { frame_index: 20, bbox: { x: 0.34, y: 0.67, w: 0.15, h: 0.09 }, confidence: 0.96 },
        { frame_index: 320, bbox: { x: 0.44, y: 0.65, w: 0.16, h: 0.09 }, confidence: 0.94 },
        { frame_index: 640, bbox: { x: 0.56, y: 0.68, w: 0.15, h: 0.09 }, confidence: 0.91 },
      ],
    },
    {
      track_id: 2,
      kind: "face",
      enabled: false,
      whitelisted: true,
      keyframes: [
        { frame_index: 0, bbox: { x: 0.12, y: 0.22, w: 0.13, h: 0.22 }, confidence: 0.98 },
        { frame_index: 420, bbox: { x: 0.15, y: 0.2, w: 0.13, h: 0.22 }, confidence: 0.97 },
        { frame_index: 820, bbox: { x: 0.13, y: 0.22, w: 0.13, h: 0.22 }, confidence: 0.97 },
      ],
    },
    {
      track_id: 3,
      kind: "face",
      enabled: true,
      whitelisted: false,
      keyframes: [
        { frame_index: 90, bbox: { x: 0.72, y: 0.26, w: 0.11, h: 0.18 }, confidence: 0.91 },
        { frame_index: 480, bbox: { x: 0.64, y: 0.25, w: 0.11, h: 0.18 }, confidence: 0.88 },
        { frame_index: 760, bbox: { x: 0.7, y: 0.27, w: 0.11, h: 0.18 }, confidence: 0.89 },
      ],
    },
    {
      track_id: 4,
      kind: "screen_pii",
      enabled: true,
      whitelisted: false,
      keyframes: [
        { frame_index: 460, bbox: { x: 0.64, y: 0.08, w: 0.29, h: 0.11 }, confidence: 0.93 },
        { frame_index: 810, bbox: { x: 0.64, y: 0.08, w: 0.29, h: 0.11 }, confidence: 0.92 },
      ],
    },
  ];
}

// ============================================================================
// Reactive Studio State Store
// ============================================================================

export function createStudioState() {
  const demoProject = sampleProject();
  const demoTranscript = sampleTranscript();
  const firstSeg = demoTranscript.segments[0]?.id ?? null;

  const initialTheme: StudioTheme = detectInitialIsLight() ? "light" : "dark";
  applyThemeToDocument(initialTheme === "light", false);

  // Active Project & Media
  const [project, setProject] = createSignal<Project>(demoProject);
  const [recentProjects, setRecentProjects] = createSignal<Project[]>([demoProject]);
  const [isDemoMedia, setIsDemoMedia] = createSignal<boolean>(true);
  const [dirty, setDirty] = createSignal<boolean>(false);
  const [previewProxyPath, setPreviewProxyPath] = createSignal<string | null>(null);
  const [waveformPeaks, setWaveformPeaks] = createSignal<number[] | null>(null);

  // Playback & Selection
  const [currentMs, setCurrentMs] = createSignal<number>(1_200);
  const [playing, setPlaying] = createSignal<boolean>(false);
  const [playbackRate, setPlaybackRate] = createSignal<number>(1.0);
  const [selectedSegmentId, setSelectedSegmentId] = createSignal<string | null>(firstSeg);
  const [selectedTrackId, setSelectedTrackId] = createSignal<number | null>(1);

  // Workspace Mode & UI Panels
  const [mode, setMode] = createSignal<StudioMode>("subtitles");
  const [theme, setTheme] = createSignal<StudioTheme>(initialTheme);
  const [showExportDrawer, setShowExportDrawer] = createSignal<boolean>(false);
  const [showProjectMenu, setShowProjectMenu] = createSignal<boolean>(false);
  const [showPrivacyMenu, setShowPrivacyMenu] = createSignal<boolean>(false);
  const [showViewMenu, setShowViewMenu] = createSignal<boolean>(false);
  const [showShortcutsModal, setShowShortcutsModal] = createSignal<boolean>(false);
  const [editingText, setEditingText] = createSignal<boolean>(false);

  // Subtitles & Glossary
  const [transcript, setTranscript] = createSignal<Transcript>(demoTranscript);
  const [showDiffs, setShowDiffs] = createSignal<boolean>(true);
  const [showSubtitlesOverlay, setShowSubtitlesOverlay] = createSignal<boolean>(true);
  const [subFilter, setSubFilter] = createSignal<string>("");
  const [subOnlyPii, setSubOnlyPii] = createSignal<boolean>(false);
  const [glossary, setGlossary] = createSignal<GlossaryEntry[]>(sampleGlossary());
  const [beepedSegments, setBeepedSegments] = createSignal<Set<string>>(new Set());

  // Timeline View
  const [timelineZoom, setTimelineZoom] = createSignal<number>(76.0);
  const [timelineSnap, setTimelineSnap] = createSignal<boolean>(true);

  // Privacy Shield
  const [shieldCfg, setShieldCfg] = createSignal<ShieldConfig>({
    detect_plates: true,
    detect_faces: true,
    detect_screen_pii: true,
    detect_qr: true,
    blur_style: "gaussian",
    spatial_padding_ratio: 0.1,
    temporal_padding_frames: 3,
  });
  const [tracks, setTracks] = createSignal<RedactionTrack[]>(sampleTracks());
  const [trackVerdicts, setTrackVerdicts] = createSignal<Record<number, TrackVerdict>>({
    2: "whitelisted",
  });
  const [showMasks, setShowMasks] = createSignal<boolean>(true);
  const [showZeroLeakMargin, setShowZeroLeakMargin] = createSignal<boolean>(true);
  const [optInTelemetry, setOptInTelemetry] = createSignal<boolean>(false);

  // Export Drawer
  const [exportFormat, setExportFormat] = createSignal<ExportFormat>("srt");
  const [exportStripFillers, setExportStripFillers] = createSignal<boolean>(true);
  const [exportIncludeSpeaker, setExportIncludeSpeaker] = createSignal<boolean>(false);
  const [exportMaxChars, setExportMaxChars] = createSignal<number>(42);
  const [exportCopied, setExportCopied] = createSignal<boolean>(false);

  // Local Lightweight Models
  const [models, setModels] = createSignal<ModelInstallStatus[]>(
    defaultLightweightModels().map((spec) => ({
      spec,
      installed: false,
      local_path: null,
      bytes_on_disk: 0,
    }))
  );

  // Status, Jobs & Toasts
  const [busyJob, setBusyJob] = createSignal<string | null>(null);
  const [statusMsg, setStatusMsg] = createSignal<[ToastSeverity, string]>([
    "success",
    "Ready — 100% local & offline (Street Vlog 4K Demo)",
  ]);
  const [toasts, setToasts] = createSignal<ToastItem[]>([]);
  let nextToastId = 1;

  // Undo / Redo History
  const [undoStack, setUndoStack] = createSignal<UndoSnapshot[]>([]);
  const [redoStack, setRedoStack] = createSignal<UndoSnapshot[]>([]);

  function snapshot(): UndoSnapshot {
    return {
      transcript: structuredClone(transcript()),
      tracks: structuredClone(tracks()),
      trackVerdicts: { ...trackVerdicts() },
      beepedSegments: Array.from(beepedSegments()),
    };
  }

  function pushUndo(): void {
    const snap = snapshot();
    setUndoStack((prev) => {
      const next = [...prev, snap];
      return next.length > 50 ? next.slice(next.length - 50) : next;
    });
    setRedoStack([]);
    setDirty(true);
  }

  function undo(): void {
    const stack = undoStack();
    if (stack.length === 0) return;
    const prev = stack[stack.length - 1];
    setUndoStack(stack.slice(0, -1));
    setRedoStack((r) => [...r, snapshot()]);
    setTranscript(prev.transcript);
    setTracks(prev.tracks);
    setTrackVerdicts(prev.trackVerdicts);
    setBeepedSegments(new Set(prev.beepedSegments));
    notify("info", "Undid last edit");
  }

  function redo(): void {
    const stack = redoStack();
    if (stack.length === 0) return;
    const next = stack[stack.length - 1];
    setRedoStack(stack.slice(0, -1));
    setUndoStack((u) => [...u, snapshot()]);
    setTranscript(next.transcript);
    setTracks(next.tracks);
    setTrackVerdicts(next.trackVerdicts);
    setBeepedSegments(new Set(next.beepedSegments));
    notify("info", "Redid edit");
  }

  function notify(severity: ToastSeverity, message: string): void {
    setStatusMsg([severity, message]);
    const id = nextToastId++;
    setToasts((prev) => {
      const next = [...prev, { id, severity, message }];
      return next.length > 3 ? next.slice(next.length - 3) : next;
    });
  }

  function dismissToast(id: number): void {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }

  function seekMs(ms: number): void {
    const dur = Math.max(1, project().media.duration_ms);
    setCurrentMs(Math.max(0, Math.min(dur, Math.round(ms))));
  }

  function togglePlay(): void {
    const now = playing();
    if (!now) {
      const dur = project().media.duration_ms;
      if (currentMs() >= Math.max(0, dur - 50)) {
        setCurrentMs(0);
      }
    }
    setPlaying(!now);
  }

  function toggleTheme(): void {
    const next: StudioTheme = theme() === "light" ? "dark" : "light";
    setTheme(next);
    applyThemeToDocument(next === "light", true);
  }

  function closeOverlays(): void {
    setShowExportDrawer(false);
    setShowProjectMenu(false);
    setShowPrivacyMenu(false);
    setShowViewMenu(false);
    setShowShortcutsModal(false);
  }

  /** Hydrates preview proxy and real waveform peaks for a real local media file in Tauri. */
  function hydrateMediaAssets(media: MediaRef): void {
    setPreviewProxyPath(null);
    setWaveformPeaks(null);
    const mediaPath = media.path;
    if (!isTauriRuntime() || !mediaPath.trim()) return;

    void commands
      .preparePreviewProxy(media)
      .then((proxy) => {
        if (proxy && project().media.path === mediaPath) {
          setPreviewProxyPath(proxy);
        }
      })
      .catch(() => {
        // Fallback to direct source path via convertFileSrc
      });

    void commands
      .extractWaveform(media, 160)
      .then((peaks) => {
        if (peaks.length > 0 && project().media.path === mediaPath) {
          setWaveformPeaks(peaks);
        }
      })
      .catch(() => {
        // Fallback to synthetic waveform
      });
  }

  function loadDemo(): void {
    const proj = sampleProject();
    const tr = sampleTranscript();
    setProject(proj);
    setIsDemoMedia(true);
    setPreviewProxyPath(null);
    setWaveformPeaks(null);
    setTranscript(tr);
    setTracks(sampleTracks());
    setTrackVerdicts({ 2: "whitelisted" });
    setBeepedSegments(new Set<string>());
    setSelectedSegmentId(tr.segments[0]?.id ?? null);
    setSelectedTrackId(1);
    setCurrentMs(1_200);
    setPlaying(false);
    setDirty(false);
    notify("info", "Loaded interactive 4K Street Vlog demo project");
  }

  return {
    // Active Project & Media
    project,
    setProject,
    recentProjects,
    setRecentProjects,
    isDemoMedia,
    setIsDemoMedia,
    dirty,
    setDirty,
    previewProxyPath,
    setPreviewProxyPath,
    waveformPeaks,
    setWaveformPeaks,
    hydrateMediaAssets,

    // Playback & Selection
    currentMs,
    setCurrentMs,
    playing,
    setPlaying,
    playbackRate,
    setPlaybackRate,
    selectedSegmentId,
    setSelectedSegmentId,
    selectedTrackId,
    setSelectedTrackId,

    // Workspace Mode & UI Panels
    mode,
    setMode,
    theme,
    setTheme,
    showExportDrawer,
    setShowExportDrawer,
    showProjectMenu,
    setShowProjectMenu,
    showPrivacyMenu,
    setShowPrivacyMenu,
    showViewMenu,
    setShowViewMenu,
    showShortcutsModal,
    setShowShortcutsModal,
    editingText,
    setEditingText,

    // Subtitles & Glossary
    transcript,
    setTranscript,
    showDiffs,
    setShowDiffs,
    showSubtitlesOverlay,
    setShowSubtitlesOverlay,
    subFilter,
    setSubFilter,
    subOnlyPii,
    setSubOnlyPii,
    glossary,
    setGlossary,
    beepedSegments,
    setBeepedSegments,

    // Timeline View
    timelineZoom,
    setTimelineZoom,
    timelineSnap,
    setTimelineSnap,

    // Privacy Shield
    shieldCfg,
    setShieldCfg,
    tracks,
    setTracks,
    trackVerdicts,
    setTrackVerdicts,
    showMasks,
    setShowMasks,
    showZeroLeakMargin,
    setShowZeroLeakMargin,
    optInTelemetry,
    setOptInTelemetry,

    // Export Drawer
    exportFormat,
    setExportFormat,
    exportStripFillers,
    setExportStripFillers,
    exportIncludeSpeaker,
    setExportIncludeSpeaker,
    exportMaxChars,
    setExportMaxChars,
    exportCopied,
    setExportCopied,

    // Local Lightweight Models
    models,
    setModels,

    // Status, Jobs & Toasts
    busyJob,
    setBusyJob,
    statusMsg,
    setStatusMsg,
    toasts,
    dismissToast,
    notify,

    // Undo / Redo
    undoStack,
    redoStack,
    pushUndo,
    undo,
    redo,

    // Actions
    seekMs,
    togglePlay,
    toggleTheme,
    closeOverlays,
    loadDemo,
  };
}

export type StudioState = ReturnType<typeof createStudioState>;

export const StudioContext = createContext<StudioState>();

export function useStudio(): StudioState {
  const ctx = useContext(StudioContext);
  if (!ctx) {
    throw new Error("useStudio must be called within StudioContext.Provider");
  }
  return ctx;
}
