import { Match, Switch, onCleanup, onMount, type Component } from "solid-js";
import { commands, isTauriRuntime } from "./bridge";
import { ExportDrawer } from "./components/ExportDrawer";
import { GlossaryInspector } from "./components/GlossaryInspector";
import { MediaInspector } from "./components/MediaInspector";
import { NavRail } from "./components/NavRail";
import { ShortcutsModal, StatusBar, ToastStack } from "./components/Overlays";
import { ShieldInspector } from "./components/ShieldInspector";
import { Stage } from "./components/Stage";
import { StudioBar } from "./components/StudioBar";
import { SubtitlesInspector } from "./components/SubtitlesInspector";
import { Timeline } from "./components/Timeline";
import { StudioContext, createStudioState } from "./state";

const StudioWorkspace: Component = () => {
  const state = createStudioState();

  onMount(() => {
    // Hydrate persisted SQLite glossary, recent projects & model statuses when in Tauri
    if (isTauriRuntime()) {
      void commands
        .listGlossary()
        .then((entries) => {
          if (entries.length > 0) state.setGlossary(entries);
        })
        .catch(() => {});

      void commands
        .listRecentProjects()
        .then((projects) => {
          if (projects.length > 0) state.setRecentProjects(projects);
        })
        .catch(() => {});

      void commands
        .listModels()
        .then((models) => {
          if (models.length > 0) state.setModels(models);
        })
        .catch(() => {});
    }

    // Smooth 60fps requestAnimationFrame ticker for demo playback
    let lastTs = performance.now();
    let rafId = 0;

    const tick = (ts: number) => {
      const dtMs = Math.max(0, Math.min(100, ts - lastTs));
      lastTs = ts;

      if (state.playing() && state.isDemoMedia()) {
        const rate = state.playbackRate();
        const dur = Math.max(1, state.project().media.duration_ms);
        const next = Math.round(state.currentMs() + dtMs * rate);
        if (next >= dur) {
          state.setCurrentMs(dur);
          state.setPlaying(false);
        } else {
          state.setCurrentMs(next);
        }
      }

      rafId = window.requestAnimationFrame(tick);
    };
    rafId = window.requestAnimationFrame(tick);

    // Global NLE Keyboard Shortcuts (respecting text editing & IME composition)
    const onKeyDown = (evt: KeyboardEvent) => {
      if (evt.isComposing || evt.keyCode === 229) return;

      const key = evt.key;
      const lower = key.toLowerCase();
      const cmdOrCtrl = evt.ctrlKey || evt.metaKey;
      const target = evt.target as HTMLElement | null;
      const inInput =
        target !== null &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable);
      const editing = state.editingText() || inInput;

      if (key === "Escape") {
        state.closeOverlays();
        return;
      }

      if (cmdOrCtrl) {
        if (lower === "e") {
          evt.preventDefault();
          const cur = state.showExportDrawer();
          state.closeOverlays();
          state.setShowExportDrawer(!cur);
          return;
        }
        if (lower === "o") {
          evt.preventDefault();
          state.setMode("media");
          return;
        }
        if (lower === "l" && evt.shiftKey) {
          evt.preventDefault();
          state.toggleTheme();
          return;
        }
        if (key === "=" || key === "+") {
          evt.preventDefault();
          state.setTimelineZoom(Math.min(240, state.timelineZoom() + 20));
          return;
        }
        if (key === "-") {
          evt.preventDefault();
          state.setTimelineZoom(Math.max(32, state.timelineZoom() - 20));
          return;
        }
        if (lower === "z" && !editing) {
          evt.preventDefault();
          if (evt.shiftKey) state.redo();
          else state.undo();
          return;
        }
      }

      if (editing) return;

      if (key === " ") {
        evt.preventDefault();
        state.togglePlay();
      } else if (lower === "k") {
        state.setPlaying(false);
        state.setPlaybackRate(1.0);
      } else if (lower === "l") {
        const nextR = state.playing()
          ? Math.min(2.0, state.playbackRate() * 1.5)
          : 1.0;
        state.setPlaybackRate(nextR);
        state.setPlaying(true);
      } else if (lower === "j") {
        state.setPlaying(false);
        state.seekMs(Math.max(0, state.currentMs() - 1_000));
      } else if (lower === "d") {
        state.setShowDiffs(!state.showDiffs());
      } else if (lower === "m") {
        state.setShowMasks(!state.showMasks());
      } else if (key === "?") {
        const cur = state.showShortcutsModal();
        state.closeOverlays();
        state.setShowShortcutsModal(!cur);
      } else if (lower === "w") {
        const tid = state.selectedTrackId();
        if (tid !== null) {
          let changed = false;
          let becomingWl = false;
          state.pushUndo();
          state.setTracks((prev) =>
            prev.map((t) => {
              if (t.track_id === tid && t.kind === "face") {
                becomingWl = !t.whitelisted;
                changed = true;
                return { ...t, whitelisted: becomingWl, enabled: !becomingWl };
              }
              return t;
            })
          );
          if (changed) {
            if (becomingWl) {
              state.setTrackVerdicts((v) => ({ ...v, [tid]: "whitelisted" }));
              state.notify("success", `Whitelisted Creator Face #${tid}`);
            } else {
              state.setTrackVerdicts((v) => {
                const next = { ...v };
                delete next[tid];
                return next;
              });
              state.notify("info", `Un-whitelisted Face #${tid}`);
            }
          }
        }
      } else if (lower === "n") {
        const tid = state.selectedTrackId();
        if (tid !== null) {
          state.pushUndo();
          state.setTracks((prev) =>
            prev.map((t) =>
              t.track_id === tid
                ? { ...t, enabled: false, whitelisted: false }
                : t
            )
          );
          state.setTrackVerdicts((v) => ({ ...v, [tid]: "not_pii" }));
          state.notify("info", `Marked Track #${tid} as Not PII`);
        }
      } else if (key === "ArrowUp" || key === "ArrowDown") {
        const segs = state.transcript().segments;
        if (segs.length === 0) return;
        evt.preventDefault();
        const curId = state.selectedSegmentId();
        const curIdx = curId
          ? Math.max(
              0,
              segs.findIndex((s) => s.id === curId)
            )
          : 0;
        const nextIdx =
          key === "ArrowDown"
            ? Math.min(segs.length - 1, curIdx + 1)
            : Math.max(0, curIdx - 1);
        const seg = segs[nextIdx];
        if (seg) {
          state.setSelectedSegmentId(seg.id);
          state.seekMs(seg.start_ms);
        }
      }
    };

    window.addEventListener("keydown", onKeyDown);
    onCleanup(() => {
      window.cancelAnimationFrame(rafId);
      window.removeEventListener("keydown", onKeyDown);
    });
  });

  return (
    <StudioContext.Provider value={state}>
      <div class="tc-studio">
        <StudioBar />
        <NavRail />
        <Stage />

        <aside class="tc-inspector">
          <Switch>
            <Match when={state.mode() === "subtitles"}>
              <SubtitlesInspector />
            </Match>
            <Match when={state.mode() === "shield"}>
              <ShieldInspector />
            </Match>
            <Match when={state.mode() === "glossary"}>
              <GlossaryInspector />
            </Match>
            <Match when={state.mode() === "media"}>
              <MediaInspector />
            </Match>
          </Switch>
        </aside>

        <Timeline />
        <StatusBar />
        <ExportDrawer />
        <ShortcutsModal />
        <ToastStack />
      </div>
    </StudioContext.Provider>
  );
};

export default StudioWorkspace;
