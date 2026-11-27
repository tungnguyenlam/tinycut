import {
  For,
  Show,
  createEffect,
  createMemo,
  createSignal,
  type Component,
} from "solid-js";
import {
  commands,
  copyToClipboard,
  downloadTextWeb,
  isTauriRuntime,
} from "../bridge";
import { prepareExportTranscript, renderExportLocal } from "../domainExt";
import {
  EXPORT_FORMATS,
  getExportFormatInfo,
  useStudio,
} from "../state";
import { IconClose, IconExport, IconShield } from "./Icons";

export const ExportDrawer: Component = () => {
  const state = useStudio();

  const fmtInfo = () => getExportFormatInfo(state.exportFormat());
  const [savePath, setSavePath] = createSignal(
    `./${state.project().title}.${fmtInfo().extension}`
  );
  const [videoOutPath, setVideoOutPath] = createSignal(
    `./${state.project().title}_redacted.mp4`
  );
  const [exportingVideo, setExportingVideo] = createSignal(false);

  createEffect(() => {
    const title = state.project().title;
    const ext = fmtInfo().extension;
    setSavePath(`./${title}.${ext}`);
  });

  createEffect(() => {
    const title = state.project().title;
    setVideoOutPath(`./${title}_redacted.mp4`);
  });

  const prepared = createMemo(() =>
    prepareExportTranscript(
      state.transcript(),
      state.exportStripFillers(),
      state.exportIncludeSpeaker(),
      state.exportMaxChars()
    )
  );

  const previewText = createMemo(() =>
    renderExportLocal(prepared(), state.project().media, state.exportFormat())
  );

  const lines = createMemo(() => previewText().split("\n"));

  const activeMasks = createMemo(
    () => state.tracks().filter((t) => t.enabled && !t.whitelisted).length
  );

  async function onCopy(): Promise<void> {
    try {
      await copyToClipboard(previewText());
      state.setExportCopied(true);
      state.notify("success", `Copied ${fmtInfo().label} to clipboard`);
    } catch (e) {
      state.notify("error", `Copy failed: ${String(e)}`);
    }
  }

  async function onSave(): Promise<void> {
    const targetPath = savePath().trim();
    const fname = `${state.project().title}.${fmtInfo().extension}`;
    const fallbackText = previewText();

    if (isTauriRuntime()) {
      try {
        let content = fallbackText;
        try {
          content = await commands.exportSubtitles(
            prepared(),
            state.project().media,
            state.exportFormat()
          );
        } catch {
          // Use local preview fallback
        }
        const saved = await commands.saveTextFile(targetPath, content);
        state.notify("success", `Exported ${fmtInfo().label} → ${saved}`);
      } catch (e) {
        state.notify("error", `Save failed: ${String(e)}`);
      }
    } else {
      try {
        downloadTextWeb(fname, fallbackText);
        state.notify("success", `Downloaded ${fname}`);
      } catch (e) {
        state.notify("error", `Download failed: ${String(e)}`);
      }
    }
  }

  async function onExportRedactedVideo(): Promise<void> {
    const outPath = videoOutPath().trim();
    if (!outPath) {
      state.notify("warning", "Enter an output MP4 path");
      return;
    }
    if (!isTauriRuntime()) {
      state.notify(
        "info",
        "Burn-in MP4 export requires the TinyCut desktop app (FFmpeg local pipeline)."
      );
      return;
    }
    setExportingVideo(true);
    state.setBusyJob("Exporting Redacted MP4");
    try {
      const saved = await commands.exportRedactedVideo(
        state.project().media,
        state.tracks(),
        state.shieldCfg(),
        outPath
      );
      state.notify("success", `Exported redacted MP4 → ${saved}`);
    } catch (e) {
      state.notify("error", `Video export failed: ${String(e)}`);
    } finally {
      setExportingVideo(false);
      state.setBusyJob(null);
    }
  }

  return (
    <Show when={state.showExportDrawer()}>
      <div
        class="tc-backdrop"
        onClick={() => state.setShowExportDrawer(false)}
      />

      <div
        class="tc-sheet tc-glass-thick"
        role="dialog"
        aria-label="Export Studio Output"
      >
        <div class="tc-sheet__head">
          <div>
            <div class="tc-sheet__title">Export</div>
            <div class="tc-sheet__sub">
              {state.project().title} · {state.transcript().segments.length}{" "}
              subtitles · {activeMasks()} active masks
            </div>
          </div>
          <button
            class="tc-btn tc-btn--ghost tc-btn--icon"
            onClick={() => state.setShowExportDrawer(false)}
          >
            <IconClose />
          </button>
        </div>

        <div class="tc-sheet__body">
          {/* Format Radio List */}
          <div class="tc-group">
            <div class="tc-group__title">Subtitle Format</div>
            <div class="tc-radio-list">
              <For each={EXPORT_FORMATS}>
                {(fmt) => (
                  <button
                    class={
                      state.exportFormat() === fmt.id
                        ? "tc-radio-row tc-radio-row--active"
                        : "tc-radio-row"
                    }
                    onClick={() => {
                      state.setExportFormat(fmt.id);
                      state.setExportCopied(false);
                    }}
                  >
                    <div class="tc-flex-row">
                      <span class="tc-radio-dot" />
                      <span>{fmt.label}</span>
                    </div>
                    <span class="tc-form-row__sub">{fmt.hint}</span>
                  </button>
                )}
              </For>
            </div>
          </div>

          {/* Formatting Options */}
          <div class="tc-group">
            <div class="tc-group__title">Options</div>
            <div class="tc-group__card">
              <div class="tc-form-row">
                <span class="tc-form-row__label">Strip speech fillers</span>
                <button
                  class={
                    state.exportStripFillers()
                      ? "tc-switch tc-switch--on"
                      : "tc-switch"
                  }
                  role="switch"
                  aria-checked={state.exportStripFillers()}
                  onClick={() =>
                    state.setExportStripFillers(!state.exportStripFillers())
                  }
                />
              </div>
              <div class="tc-form-row">
                <span class="tc-form-row__label">Include speaker label</span>
                <button
                  class={
                    state.exportIncludeSpeaker()
                      ? "tc-switch tc-switch--on"
                      : "tc-switch"
                  }
                  role="switch"
                  aria-checked={state.exportIncludeSpeaker()}
                  onClick={() =>
                    state.setExportIncludeSpeaker(!state.exportIncludeSpeaker())
                  }
                />
              </div>
              <div class="tc-form-row">
                <span class="tc-form-row__label">Line wrap width</span>
                <div class="tc-stepper">
                  <button
                    class="tc-stepper__btn"
                    onClick={() =>
                      state.setExportMaxChars(
                        Math.max(24, state.exportMaxChars() - 4)
                      )
                    }
                  >
                    −
                  </button>
                  <span class="tc-stepper__val tc-mono">
                    {state.exportMaxChars()} ch
                  </span>
                  <button
                    class="tc-stepper__btn"
                    onClick={() =>
                      state.setExportMaxChars(
                        Math.min(80, state.exportMaxChars() + 4)
                      )
                    }
                  >
                    +
                  </button>
                </div>
              </div>
            </div>
          </div>

          <Show when={isTauriRuntime()}>
            <div class="tc-group">
              <div class="tc-group__title">Subtitle Destination Path</div>
              <input
                class="tc-input tc-mono"
                value={savePath()}
                onFocus={() => state.setEditingText(true)}
                onBlur={() => state.setEditingText(false)}
                onInput={(e) => setSavePath(e.currentTarget.value)}
              />
            </div>
          </Show>

          {/* Live Preview */}
          <div class="tc-group">
            <div class="tc-group__title">
              <span>Preview ({fmtInfo().label})</span>
              <span class="tc-group__sub tc-mono">{lines().length} lines</span>
            </div>
            <div class="tc-code-box">
              <For each={lines()}>
                {(line, idx) => {
                  const lineCls = () => {
                    if (
                      line.includes("-->") ||
                      line.startsWith("Dialogue:") ||
                      line.startsWith("00")
                    ) {
                      return "tc-code-line__text tc-code-line__text--tc";
                    }
                    if (
                      line.trimStart().startsWith("<") ||
                      line.startsWith("[")
                    ) {
                      return "tc-code-line__text tc-code-line__text--tag";
                    }
                    return "tc-code-line__text";
                  };
                  return (
                    <div class="tc-code-line">
                      <span class="tc-code-line__num">{idx() + 1}</span>
                      <span class={lineCls()}>{line}</span>
                    </div>
                  );
                }}
              </For>
            </div>
          </div>

          <div class="tc-export-section-sep" />

          {/* Burn-In Redacted Video (MP4) Export Section */}
          <div class="tc-group">
            <div class="tc-group__title">
              <span>Burn-In Redacted Video (MP4)</span>
              <span class="tc-group__sub tc-mono">
                {activeMasks()} mask(s) · {state.shieldCfg().blur_style}
              </span>
            </div>
            <div class="tc-group__card">
              <div class="tc-form-row tc-form-row--stack">
                <input
                  class="tc-input tc-mono"
                  value={videoOutPath()}
                  placeholder="./output_redacted.mp4"
                  onFocus={() => state.setEditingText(true)}
                  onBlur={() => state.setEditingText(false)}
                  onInput={(e) => setVideoOutPath(e.currentTarget.value)}
                />
              </div>
              <div class="tc-burnin-actions">
                <span class="tc-form-row__sub">
                  Zero-Leak +{Math.round(state.shieldCfg().spatial_padding_ratio * 100)}% · ±
                  {state.shieldCfg().temporal_padding_frames}f
                </span>
                <button
                  class="tc-btn tc-btn--primary tc-btn--sm"
                  disabled={exportingVideo() || state.busyJob() !== null}
                  onClick={() => void onExportRedactedVideo()}
                >
                  <IconShield />
                  {exportingVideo() ? "Rendering MP4…" : "Export Redacted MP4"}
                </button>
              </div>
            </div>
          </div>
        </div>

        <div class="tc-sheet__foot">
          <button class="tc-btn" onClick={() => void onCopy()}>
            {state.exportCopied() ? "Copied" : "Copy"}
          </button>
          <button
            class="tc-btn tc-btn--primary"
            onClick={() => void onSave()}
          >
            <IconExport />
            {isTauriRuntime()
              ? "Save to Disk"
              : `Download .${fmtInfo().extension}`}
          </button>
        </div>
      </div>
    </Show>
  );
};
