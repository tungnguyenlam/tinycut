import { For, Show, type Component } from "solid-js";
import { msToFrame } from "../domainExt";
import { useStudio, type ToastSeverity } from "../state";
import { IconAlert, IconCheck, IconClose } from "./Icons";

function statusMsgClass(sev: ToastSeverity): string {
  switch (sev) {
    case "info":
      return "tc-status__msg--info";
    case "success":
      return "tc-status__msg--success";
    case "warning":
      return "tc-status__msg--warning";
    case "error":
      return "tc-status__msg--error";
  }
}

function toastIconClass(sev: ToastSeverity): string {
  switch (sev) {
    case "info":
      return "tc-toast__icon tc-toast__icon--info";
    case "success":
      return "tc-toast__icon tc-toast__icon--success";
    case "warning":
      return "tc-toast__icon tc-toast__icon--warning";
    case "error":
      return "tc-toast__icon tc-toast__icon--error";
  }
}

export const StatusBar: Component = () => {
  const state = useStudio();

  const fps = () => Math.max(1, state.project().media.fps);
  const curFrame = () => msToFrame(state.currentMs(), fps());
  const totalFrames = () =>
    msToFrame(Math.max(1, state.project().media.duration_ms), fps());
  const activeMasks = () =>
    state.tracks().filter((t) => t.enabled && !t.whitelisted).length;

  return (
    <footer class="tc-status">
      <div class="tc-status__group">
        <Show
          when={state.busyJob()}
          fallback={
            <span class={statusMsgClass(state.statusMsg()[0])}>
              {state.statusMsg()[1]}
            </span>
          }
        >
          {(job) => <span>{job()}…</span>}
        </Show>
      </div>

      <div class="tc-status__group tc-mono">
        <span>
          Frame {curFrame()} / {totalFrames()}
        </span>
        <span class="tc-status__sep">·</span>
        <span>{activeMasks()} shield</span>
        <span class="tc-status__sep">·</span>
        <span>
          {state.project().inference_mode === "local" ? "Local" : "Cloud Boost"}
        </span>
      </div>
    </footer>
  );
};

export const ToastStack: Component = () => {
  const state = useStudio();

  return (
    <Show when={state.toasts().length > 0}>
      <div class="tc-toasts">
        <For each={state.toasts()}>
          {(t) => (
            <div class="tc-toast tc-glass-thick">
              <span class={toastIconClass(t.severity)}>
                <Show when={t.severity === "success"} fallback={<IconAlert />}>
                  <IconCheck />
                </Show>
              </span>
              <span>{t.message}</span>
              <button
                class="tc-btn tc-btn--ghost tc-btn--sm tc-btn--icon"
                onClick={() => state.dismissToast(t.id)}
              >
                <IconClose />
              </button>
            </div>
          )}
        </For>
      </div>
    </Show>
  );
};

const SHORTCUTS: [string, string][] = [
  ["Play / Pause", "Space"],
  ["Reverse / Pause / Forward", "J / K / L"],
  ["Prev / Next Subtitle", "↑ / ↓"],
  ["Toggle Inline Diff", "D"],
  ["Toggle Blur Masks", "M"],
  ["Whitelist Selected Face", "W"],
  ["Mark Track Not PII", "N"],
  ["Timeline Zoom In / Out", "⌘+ / ⌘−"],
  ["Switch Light / Dark", "⌘⇧L"],
  ["Export", "⌘E"],
  ["Undo / Redo", "⌘Z / ⌘⇧Z"],
  ["Close Sheet / Popover", "Esc"],
];

export const ShortcutsModal: Component = () => {
  const state = useStudio();

  return (
    <Show when={state.showShortcutsModal()}>
      <div
        class="tc-backdrop"
        onClick={() => state.setShowShortcutsModal(false)}
      />
      <div
        class="tc-sheet tc-sheet--compact tc-glass-thick"
        role="dialog"
        aria-label="Keyboard Shortcuts"
      >
        <div class="tc-sheet__head">
          <div>
            <div class="tc-sheet__title">Keyboard Shortcuts</div>
            <div class="tc-sheet__sub">
              Single-key shortcuts pause automatically while editing text.
            </div>
          </div>
          <button
            class="tc-btn tc-btn--ghost tc-btn--icon"
            onClick={() => state.setShowShortcutsModal(false)}
          >
            <IconClose />
          </button>
        </div>

        <div class="tc-sheet__body">
          <div class="tc-shortcut-grid">
            <For each={SHORTCUTS}>
              {([desc, keys]) => (
                <div class="tc-shortcut-row">
                  <span>{desc}</span>
                  <span class="tc-kbd">{keys}</span>
                </div>
              )}
            </For>
          </div>
        </div>
      </div>
    </Show>
  );
};
