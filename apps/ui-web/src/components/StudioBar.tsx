import { For, Show, type Component } from "solid-js";
import type { Project } from "../bindings";
import { commands, isTauriRuntime } from "../bridge";
import { formatSegTime } from "../domainExt";
import { useStudio } from "../state";
import {
  IconBrand,
  IconCheck,
  IconChevronDown,
  IconExport,
  IconLock,
  IconMoon,
  IconPlay,
  IconPlus,
  IconRedo,
  IconSun,
  IconUndo,
} from "./Icons";

export const StudioBar: Component = () => {
  const state = useStudio();

  const resLabel = () => {
    const w = state.project().media.width;
    if (w >= 3840) return "4K";
    if (w >= 1920) return "1080p";
    if (w > 0) return "HD";
    return "Audio";
  };

  const modeTitle = () => {
    switch (state.mode()) {
      case "media":
        return "Media & Projects";
      case "subtitles":
        return "Subtitles & 3-Way Diff";
      case "shield":
        return "Visual Privacy Shield";
      case "glossary":
        return "Personal Glossary";
    }
  };

  function persistProject(updated: Project): void {
    state.setProject(updated);
    state.setDirty(false);
    if (isTauriRuntime()) {
      void commands.saveProject(updated).catch(() => {});
    }
  }

  return (
    <>
      <header class="tc-bar" data-tauri-drag-region="true">
        {/* Left Floating Liquid Glass Capsule: Brand + Project Menu */}
        <div class="tc-capsule tc-glass">
          <div class="tc-brand">
            <div class="tc-brand__mark">
              <IconBrand />
            </div>
            <span>TinyCut</span>
          </div>
          <div class="tc-sep-v" />
          <button
            class="tc-proj-trigger"
            title="Switch project or import media"
            onClick={() => {
              const next = !state.showProjectMenu();
              state.closeOverlays();
              state.setShowProjectMenu(next);
            }}
          >
            <span
              class={state.dirty() ? "tc-saved-dot tc-saved-dot--dirty" : "tc-saved-dot"}
              title={state.dirty() ? "Unsaved edits" : "Saved locally"}
            />
            <span class="tc-proj-trigger__name">{state.project().title}</span>
            <span class="tc-proj-trigger__meta tc-mono">
              {resLabel()} · {Math.round(state.project().media.fps)}fps
            </span>
            <span class="tc-muted">
              <IconChevronDown />
            </span>
          </button>
        </div>

        {/* Center: Quiet Current Mode Title */}
        <div class="tc-bar__center">
          <span>{modeTitle()}</span>
        </div>

        {/* Right Floating Liquid Glass Capsule */}
        <div class="tc-capsule tc-glass">
          <button
            class="tc-privacy-btn"
            title="Inference & Privacy Posture"
            onClick={() => {
              const next = !state.showPrivacyMenu();
              state.closeOverlays();
              state.setShowPrivacyMenu(next);
            }}
          >
            <span
              class={
                state.project().inference_mode === "cloud_boost" &&
                !state.project().is_nda_private
                  ? "tc-privacy-btn__dot tc-privacy-btn__dot--cloud"
                  : "tc-privacy-btn__dot"
              }
            />
            <span>
              {state.project().inference_mode === "cloud_boost" &&
              !state.project().is_nda_private
                ? "Cloud Boost"
                : "Local"}
            </span>
            <Show when={state.project().is_nda_private}>
              <span class="tc-privacy-btn__nda">
                ·<IconLock />
                NDA
              </span>
            </Show>
            <span class="tc-muted">
              <IconChevronDown />
            </span>
          </button>

          <div class="tc-sep-v" />

          <button
            class="tc-btn tc-btn--ghost tc-btn--icon"
            disabled={state.undoStack().length === 0}
            title="Undo (Ctrl/⌘+Z)"
            onClick={() => state.undo()}
          >
            <IconUndo />
          </button>
          <button
            class="tc-btn tc-btn--ghost tc-btn--icon"
            disabled={state.redoStack().length === 0}
            title="Redo (Ctrl/⌘+Shift+Z)"
            onClick={() => state.redo()}
          >
            <IconRedo />
          </button>

          <button
            class="tc-btn tc-btn--ghost tc-btn--icon"
            title={
              state.theme() === "light"
                ? "Switch to Dark Theme (Ctrl/⌘+Shift+L)"
                : "Switch to Light Theme (Ctrl/⌘+Shift+L)"
            }
            onClick={() => state.toggleTheme()}
          >
            <Show when={state.theme() === "light"} fallback={<IconSun />}>
              <IconMoon />
            </Show>
          </button>

          <div class="tc-sep-v" />

          <button
            class="tc-btn tc-btn--primary"
            title="Export Subtitles & Video (Ctrl/⌘+E)"
            onClick={() => {
              state.closeOverlays();
              state.setShowExportDrawer(true);
            }}
          >
            <IconExport />
            Export
          </button>
        </div>
      </header>

      {/* Project Switcher Glass Popover */}
      <Show when={state.showProjectMenu()}>
        <div
          class="tc-click-away"
          onClick={() => state.setShowProjectMenu(false)}
        />
        <div class="tc-popover tc-popover--project tc-glass-thick">
          <div class="tc-popover__heading">Recent Projects</div>
          <For each={state.recentProjects()}>
            {(proj) => {
              const isCur = () => proj.id === state.project().id;
              return (
                <button
                  class={
                    isCur()
                      ? "tc-popover__item tc-popover__item--active"
                      : "tc-popover__item"
                  }
                  onClick={() => {
                    state.setProject(proj);
                    state.setIsDemoMedia(
                      proj.media.path === "/videos/saigon_street_vlog_4k.mp4"
                    );
                    if (proj.media.path !== "/videos/saigon_street_vlog_4k.mp4") {
                      state.hydrateMediaAssets(proj.media);
                    }
                    state.setShowProjectMenu(false);
                    state.notify("info", `Opened ${proj.title}`);
                  }}
                >
                  <div>
                    <div>{proj.title}</div>
                    <div class="tc-mono tc-muted">
                      {proj.media.width}×{proj.media.height} ·{" "}
                      {formatSegTime(proj.media.duration_ms)}
                    </div>
                  </div>
                  <Show when={proj.is_nda_private}>
                    <span class="tc-privacy-btn__nda">
                      <IconLock />
                    </span>
                  </Show>
                </button>
              );
            }}
          </For>
          <div class="tc-popover__sep" />
          <button
            class="tc-popover__item"
            onClick={() => {
              state.setMode("media");
              state.setShowProjectMenu(false);
            }}
          >
            <span class="tc-flex-row">
              <IconPlus />
              Import local media…
            </span>
            <span class="tc-kbd">⌘O</span>
          </button>
          <button
            class="tc-popover__item"
            onClick={() => {
              state.loadDemo();
              state.setShowProjectMenu(false);
            }}
          >
            <span class="tc-flex-row">
              <IconPlay />
              Reload 4K Street Vlog Demo
            </span>
          </button>
        </div>
      </Show>

      {/* Consolidated Privacy & Inference Glass Popover */}
      <Show when={state.showPrivacyMenu()}>
        <div
          class="tc-click-away"
          onClick={() => state.setShowPrivacyMenu(false)}
        />
        <div class="tc-popover tc-popover--privacy tc-glass-thick">
          <div class="tc-popover__heading">Inference Mode</div>

          <button
            class={
              state.project().inference_mode === "local"
                ? "tc-popover__item tc-popover__item--active"
                : "tc-popover__item"
            }
            onClick={() => {
              persistProject({ ...state.project(), inference_mode: "local" });
              state.notify("success", "Switched to Local (100% offline)");
            }}
          >
            <div>
              <div>Local On-Device</div>
              <div class="tc-form-row__sub">
                Whisper + Qwen + ONNX. Zero internet required.
              </div>
            </div>
            <Show when={state.project().inference_mode === "local"}>
              <IconCheck />
            </Show>
          </button>

          <button
            class={
              state.project().inference_mode === "cloud_boost" &&
              !state.project().is_nda_private
                ? "tc-popover__item tc-popover__item--active"
                : "tc-popover__item"
            }
            disabled={state.project().is_nda_private}
            onClick={() => {
              if (state.project().is_nda_private) return;
              persistProject({
                ...state.project(),
                inference_mode: "cloud_boost",
              });
              state.notify("info", "Cloud Boost ready (proxy-only offload)");
            }}
          >
            <div>
              <div>Cloud Boost (Pro)</div>
              <div class="tc-form-row__sub">
                {state.project().is_nda_private
                  ? "Blocked while NDA Privacy Lock is enabled."
                  : "Uploads compact Opus/720p proxy only; raw 4K stays local."}
              </div>
            </div>
            <Show
              when={
                state.project().inference_mode === "cloud_boost" &&
                !state.project().is_nda_private
              }
            >
              <IconCheck />
            </Show>
          </button>

          <div class="tc-popover__sep" />

          <div class="tc-form-row">
            <div>
              <div class="tc-form-row__label">
                <IconLock />
                NDA Privacy Lock
              </div>
              <div class="tc-form-row__sub">
                Blocks Cloud Boost &amp; all telemetry
              </div>
            </div>
            <button
              class={
                state.project().is_nda_private
                  ? "tc-switch tc-switch--warn tc-switch--on"
                  : "tc-switch tc-switch--warn"
              }
              role="switch"
              aria-checked={state.project().is_nda_private}
              onClick={() => {
                const cur = state.project();
                const nextNda = !cur.is_nda_private;
                const updated: Project = {
                  ...cur,
                  is_nda_private: nextNda,
                  inference_mode: nextNda ? "local" : cur.inference_mode,
                };
                if (nextNda) {
                  state.setOptInTelemetry(false);
                  state.notify(
                    "warning",
                    "NDA Lock ON — Cloud Boost & telemetry blocked"
                  );
                } else {
                  state.notify("info", "NDA Lock disabled");
                }
                persistProject(updated);
              }}
            />
          </div>
        </div>
      </Show>
    </>
  );
};
