import { Show, type Component } from "solid-js";
import { useStudio } from "../state";
import {
  IconGlossary,
  IconKeyboard,
  IconMedia,
  IconShield,
  IconSubtitles,
} from "./Icons";

export const NavRail: Component = () => {
  const state = useStudio();

  const segCount = () => state.transcript().segments.length;
  const activeTracks = () =>
    state.tracks().filter((t) => t.enabled && !t.whitelisted).length;
  const glossCount = () => state.glossary().length;

  return (
    <div class="tc-rail-slot">
      <nav class="tc-rail tc-glass" aria-label="Studio Navigation">
        <div class="tc-rail__group">
          <button
            class={
              state.mode() === "media"
                ? "tc-rail__btn tc-rail__btn--active"
                : "tc-rail__btn"
            }
            title="Media & Projects (Ctrl/⌘+O)"
            aria-label="Media & Projects"
            onClick={() => state.setMode("media")}
          >
            <IconMedia />
          </button>

          <button
            class={
              state.mode() === "subtitles"
                ? "tc-rail__btn tc-rail__btn--active"
                : "tc-rail__btn"
            }
            title={`Subtitles & 3-Way Diff (${segCount()})`}
            aria-label="Subtitles & 3-Way Diff"
            onClick={() => state.setMode("subtitles")}
          >
            <IconSubtitles />
            <Show when={segCount() > 0}>
              <span class="tc-rail__badge tc-mono">{segCount()}</span>
            </Show>
          </button>

          <button
            class={
              state.mode() === "shield"
                ? "tc-rail__btn tc-rail__btn--active"
                : "tc-rail__btn"
            }
            title={`Visual Privacy Shield (${activeTracks()} active)`}
            aria-label="Visual Privacy Shield"
            onClick={() => state.setMode("shield")}
          >
            <IconShield />
            <Show when={activeTracks() > 0}>
              <span class="tc-rail__badge tc-rail__badge--alert tc-mono">
                {activeTracks()}
              </span>
            </Show>
          </button>

          <button
            class={
              state.mode() === "glossary"
                ? "tc-rail__btn tc-rail__btn--active"
                : "tc-rail__btn"
            }
            title={`Personal Glossary (${glossCount()})`}
            aria-label="Personal Glossary"
            onClick={() => state.setMode("glossary")}
          >
            <IconGlossary />
            <Show when={glossCount() > 0}>
              <span class="tc-rail__badge tc-mono">{glossCount()}</span>
            </Show>
          </button>
        </div>

        <div class="tc-rail__group">
          <button
            class="tc-rail__btn"
            title="Keyboard Shortcuts (?)"
            aria-label="Keyboard Shortcuts"
            onClick={() => {
              const cur = state.showShortcutsModal();
              state.closeOverlays();
              state.setShowShortcutsModal(!cur);
            }}
          >
            <IconKeyboard />
          </button>
        </div>
      </nav>
    </div>
  );
};
