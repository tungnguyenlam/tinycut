import { For, Show, createMemo, type Component } from "solid-js";
import type { RedactionKind, RedactionTrack } from "../bindings";
import {
  detectSpokenPii,
  formatSegTime,
  frameToMs,
  msToFrame,
  trackFrameBounds,
} from "../domainExt";
import { useStudio } from "../state";
import {
  IconFace,
  IconMagnet,
  IconMonitor,
  IconPlate,
  IconSubtitles,
  IconWave,
  IconZoomIn,
  IconZoomOut,
} from "./Icons";

export const Timeline: Component = () => {
  const state = useStudio();
  let canvasEl: HTMLDivElement | undefined;
  let dragging = false;

  const durMs = () => Math.max(1, state.project().media.duration_ms);
  const fps = () => Math.max(1, state.project().media.fps);
  const totalFrames = () => Math.max(1, msToFrame(durMs(), fps()));
  const pxPerSec = () => state.timelineZoom();
  const pxPerMs = () => pxPerSec() / 1000;
  const contentWidthPx = () => Math.max(600, durMs() * pxPerMs());
  const playheadPx = () =>
    Math.max(0, Math.min(contentWidthPx(), state.currentMs() * pxPerMs()));

  const silences = createMemo(() => {
    const segs = state.transcript().segments;
    const out: [number, number][] = [];
    let cursor = 0;
    for (const seg of segs) {
      if (seg.start_ms > cursor + 250) {
        out.push([cursor, seg.start_ms]);
      }
      cursor = Math.max(cursor, seg.end_ms);
    }
    if (durMs() > cursor + 250) {
      out.push([cursor, durMs()]);
    }
    return out;
  });

  const ticks = createMemo(() => {
    const z = pxPerSec();
    const majorStep = z >= 140 ? 1_000 : z >= 65 ? 2_000 : 5_000;
    const minorStep = majorStep / 2;
    const major: number[] = [];
    const minor: number[] = [];
    const total = durMs();
    for (let t = 0; t <= total; t += majorStep) {
      major.push(t);
      if (t + minorStep < total) {
        minor.push(t + minorStep);
      }
    }
    return { major, minor };
  });

  // Mirrored SVG waveform path: real peaks from extract_waveform when available, realistic demo fallback otherwise
  const wavPath = createMemo(() => {
    const realPeaks = state.waveformPeaks();
    const segs = state.transcript().segments;
    const total = durMs();

    const samples =
      realPeaks && realPeaks.length > 1 ? realPeaks.length - 1 : 140;
    const topPts: string[] = [];
    const botPts: string[] = [];

    for (let i = 0; i <= samples; i++) {
      const ratio = i / samples;
      const x = ratio * 1000;
      let amp: number;
      if (realPeaks && realPeaks.length > 1) {
        const peak = Math.max(0, Math.min(1, realPeaks[i] ?? 0));
        amp = 2 + peak * 42;
      } else {
        const tMs = Math.round(ratio * total);
        const inSpeech = segs.some((s) => tMs >= s.start_ms && tMs <= s.end_ms);
        amp = inSpeech
          ? 10 + ((i * 37 + 13) % 24)
          : 2 + ((i * 19 + 5) % 4);
      }
      topPts.push(`${x.toFixed(1)},${(50 - amp).toFixed(1)}`);
      botPts.push(`${x.toFixed(1)},${(50 + amp).toFixed(1)}`);
    }
    botPts.reverse();
    return `M ${topPts.join(" L ")} L ${botPts.join(" L ")} Z`;
  });

  function scrubAtClientX(clientX: number): void {
    if (!canvasEl) return;
    const rect = canvasEl.getBoundingClientRect();
    if (rect.width <= 0) return;
    const x = Math.max(0, Math.min(rect.width, clientX - rect.left));
    const rawMs = Math.round(x / pxPerMs());
    const target = state.timelineSnap()
      ? Math.round(rawMs / Math.max(1, 1000 / fps())) * Math.max(1, 1000 / fps())
      : rawMs;
    state.seekMs(target);
  }

  const plateTracks = createMemo(() =>
    state.tracks().filter((t) => t.kind === "license_plate")
  );
  const faceTracks = createMemo(() =>
    state.tracks().filter((t) => t.kind === "face")
  );
  const piiTracks = createMemo(() =>
    state
      .tracks()
      .filter((t) => t.kind === "screen_pii" || t.kind === "qr_code")
  );

  function renderRedactionLane(
    laneTracks: () => RedactionTrack[],
    altBg: boolean
  ) {
    return (
      <div
        class={
          altBg
            ? "tc-tl-lane tc-tl-lane--alt tc-tl-lane--red"
            : "tc-tl-lane tc-tl-lane--red"
        }
      >
        <For each={laneTracks()}>
          {(track) => {
            const bounds = () =>
              trackFrameBounds(track, state.shieldCfg(), totalFrames());
            return (
              <Show when={bounds()}>
                {(b) => {
                  const sMs = () => frameToMs(b()[0], fps());
                  const eMs = () => Math.max(sMs() + 150, frameToMs(b()[1], fps()));
                  const psMs = () => frameToMs(b()[2], fps());
                  const peMs = () => Math.max(eMs(), frameToMs(b()[3], fps()));

                  const padLeft = () => psMs() * pxPerMs();
                  const padWidth = () =>
                    Math.max(16, (peMs() - psMs()) * pxPerMs());
                  const left = () => sMs() * pxPerMs();
                  const width = () => Math.max(20, (eMs() - sMs()) * pxPerMs());

                  const isRejected = () =>
                    state.trackVerdicts()[track.track_id] === "not_pii";
                  const isSelected = () =>
                    state.selectedTrackId() === track.track_id;

                  const kindClass = (k: RedactionKind): [string, string] => {
                    switch (k) {
                      case "license_plate":
                        return [
                          "tc-tl-rbar tc-tl-rbar--plate",
                          "tc-tl-rbar-pad tc-tl-rbar-pad--plate",
                        ];
                      case "face":
                        return [
                          "tc-tl-rbar tc-tl-rbar--face",
                          "tc-tl-rbar-pad tc-tl-rbar-pad--face",
                        ];
                      case "screen_pii":
                      case "qr_code":
                        return [
                          "tc-tl-rbar tc-tl-rbar--pii",
                          "tc-tl-rbar-pad tc-tl-rbar-pad--pii",
                        ];
                    }
                  };

                  const classes = () => {
                    if (isRejected()) {
                      return ["tc-tl-rbar tc-tl-rbar--rejected", ""];
                    }
                    if (track.whitelisted) {
                      return ["tc-tl-rbar tc-tl-rbar--whitelist", ""];
                    }
                    return kindClass(track.kind);
                  };

                  const label = () => {
                    if (track.whitelisted) return `Creator #${track.track_id}`;
                    if (isRejected()) return `Not PII #${track.track_id}`;
                    return `#${track.track_id}`;
                  };

                  return (
                    <>
                      <Show
                        when={
                          state.shieldCfg().temporal_padding_frames > 0 &&
                          classes()[1] !== ""
                        }
                      >
                        <div
                          class={classes()[1]}
                          style={{
                            left: `${padLeft().toFixed(1)}px`,
                            width: `${padWidth().toFixed(1)}px`,
                          }}
                        />
                      </Show>
                      <div
                        class={
                          isSelected()
                            ? `${classes()[0]} tc-tl-rbar--selected`
                            : classes()[0]
                        }
                        style={{
                          left: `${left().toFixed(1)}px`,
                          width: `${width().toFixed(1)}px`,
                        }}
                        onPointerDown={(e) => e.stopPropagation()}
                        onClick={(e) => {
                          e.stopPropagation();
                          state.setSelectedTrackId(track.track_id);
                          state.seekMs(sMs());
                          state.setMode("shield");
                        }}
                      >
                        {label()}
                      </div>
                    </>
                  );
                }}
              </Show>
            );
          }}
        </For>
      </div>
    );
  }

  return (
    <section class="tc-timeline">
      {/* Slim NLE Timeline Toolbar */}
      <div class="tc-tl-topbar">
        <div class="tc-tl-topbar__group">
          <button
            class="tc-btn tc-btn--ghost tc-btn--sm tc-btn--icon"
            title="Zoom out (Ctrl/⌘+-)"
            onClick={() =>
              state.setTimelineZoom(Math.max(32, state.timelineZoom() - 20))
            }
          >
            <IconZoomOut />
          </button>
          <input
            class="tc-zoom-slider"
            type="range"
            min="32"
            max="240"
            step="4"
            value={Math.round(pxPerSec())}
            title={`Timeline zoom (${Math.round(pxPerSec())} px/s)`}
            onInput={(e) => {
              const v = Number(e.currentTarget.value);
              if (!Number.isNaN(v)) {
                state.setTimelineZoom(Math.max(32, Math.min(240, v)));
              }
            }}
          />
          <button
            class="tc-btn tc-btn--ghost tc-btn--sm tc-btn--icon"
            title="Zoom in (Ctrl/⌘++)"
            onClick={() =>
              state.setTimelineZoom(Math.min(240, state.timelineZoom() + 20))
            }
          >
            <IconZoomIn />
          </button>
          <span class="tc-sep-v" />
          <button
            class={
              state.timelineSnap()
                ? "tc-btn tc-btn--sm tc-btn--active"
                : "tc-btn tc-btn--ghost tc-btn--sm"
            }
            title="Toggle frame-snapped scrubbing"
            aria-pressed={state.timelineSnap()}
            onClick={() => state.setTimelineSnap(!state.timelineSnap())}
          >
            <IconMagnet />
            Snap
          </button>
        </div>

        <div class="tc-tl-topbar__group">
          <Show when={silences().length > 0}>
            <button
              class="tc-btn tc-btn--ghost tc-btn--sm"
              title="Preview Smart Auto-Cut silence spans"
              onClick={() =>
                state.notify(
                  "info",
                  `Detected ${silences().length} silence span(s) ready for Smart Auto-Cut`
                )
              }
            >
              Auto-cut silences ({silences().length})
            </button>
          </Show>
        </div>
      </div>

      <div class="tc-tl-body">
        {/* Fixed 156px Track Headers */}
        <div class="tc-tl-headers">
          <div class="tc-tl-header-cell tc-tl-header-cell--ruler tc-mono">
            Timecode
          </div>
          <div class="tc-tl-header-cell tc-tl-header-cell--sub">
            <span class="tc-tl-swatch tc-tl-swatch--sub" />
            <IconSubtitles />
            <span class="tc-tl-header-name">Subtitles</span>
            <span class="tc-tl-header-count tc-mono">
              {state.transcript().segments.length}
            </span>
          </div>
          <div class="tc-tl-header-cell tc-tl-header-cell--wav">
            <span class="tc-tl-swatch tc-tl-swatch--wav" />
            <IconWave />
            <span class="tc-tl-header-name">Audio</span>
          </div>
          <div class="tc-tl-header-cell tc-tl-header-cell--red">
            <span class="tc-tl-swatch tc-tl-swatch--plate" />
            <IconPlate />
            <span class="tc-tl-header-name">Plates</span>
            <span class="tc-tl-header-count tc-mono">{plateTracks().length}</span>
          </div>
          <div class="tc-tl-header-cell tc-tl-header-cell--red">
            <span class="tc-tl-swatch tc-tl-swatch--face" />
            <IconFace />
            <span class="tc-tl-header-name">Faces</span>
            <span class="tc-tl-header-count tc-mono">{faceTracks().length}</span>
          </div>
          <div class="tc-tl-header-cell tc-tl-header-cell--red">
            <span class="tc-tl-swatch tc-tl-swatch--pii" />
            <IconMonitor />
            <span class="tc-tl-header-name">Screen PII</span>
            <span class="tc-tl-header-count tc-mono">{piiTracks().length}</span>
          </div>
        </div>

        {/* Scrollable Multi-Track Canvas with Drag Scrubbing */}
        <div class="tc-tl-scroll">
          <div
            ref={canvasEl}
            id="tc-timeline-canvas"
            class="tc-tl-canvas"
            style={{ width: `${Math.round(contentWidthPx())}px` }}
            onPointerDown={(e) => {
              dragging = true;
              e.currentTarget.setPointerCapture(e.pointerId);
              scrubAtClientX(e.clientX);
            }}
            onPointerMove={(e) => {
              if (dragging) {
                scrubAtClientX(e.clientX);
              }
            }}
            onPointerUp={(e) => {
              if (dragging) {
                dragging = false;
                if (e.currentTarget.hasPointerCapture(e.pointerId)) {
                  e.currentTarget.releasePointerCapture(e.pointerId);
                }
              }
            }}
          >
            {/* 1. Zoom-Adaptive Ruler */}
            <div class="tc-tl-ruler">
              <For each={ticks().major}>
                {(tickMs) => (
                  <div
                    class="tc-tl-tick tc-mono"
                    style={{ left: `${(tickMs * pxPerMs()).toFixed(1)}px` }}
                  >
                    {formatSegTime(tickMs)}
                  </div>
                )}
              </For>
              <For each={ticks().minor}>
                {(tickMs) => (
                  <div
                    class="tc-tl-tick tc-tl-tick--minor"
                    style={{ left: `${(tickMs * pxPerMs()).toFixed(1)}px` }}
                  />
                )}
              </For>
            </div>

            {/* 2. Subtitle Lane */}
            <div class="tc-tl-lane tc-tl-lane--sub">
              <For each={state.transcript().segments}>
                {(seg) => {
                  const left = () => seg.start_ms * pxPerMs();
                  const width = () =>
                    Math.max(16, (seg.end_ms - seg.start_ms) * pxPerMs());
                  const isActive = () =>
                    state.selectedSegmentId() === seg.id ||
                    (state.currentMs() >= seg.start_ms &&
                      state.currentMs() <= seg.end_ms);
                  return (
                    <div
                      class={
                        isActive()
                          ? "tc-tl-sub-clip tc-tl-sub-clip--active"
                          : "tc-tl-sub-clip"
                      }
                      style={{
                        left: `${left().toFixed(1)}px`,
                        width: `${width().toFixed(1)}px`,
                      }}
                      title={seg.text}
                      onPointerDown={(e) => e.stopPropagation()}
                      onClick={(e) => {
                        e.stopPropagation();
                        state.setSelectedSegmentId(seg.id);
                        state.seekMs(seg.start_ms);
                        state.setMode("subtitles");
                      }}
                    >
                      {seg.text}
                    </div>
                  );
                }}
              </For>
            </div>

            {/* 3. Audio Waveform + Silence Spans + Spoken PII Pins */}
            <div class="tc-tl-lane tc-tl-lane--alt tc-tl-lane--wav">
              <svg
                class="tc-tl-wav-svg"
                viewBox="0 0 1000 100"
                preserveAspectRatio="none"
              >
                <path d={wavPath()} fill="currentColor" />
              </svg>
              <For each={silences()}>
                {([sStart, sEnd]) => (
                  <div
                    class="tc-tl-silence"
                    style={{
                      left: `${(sStart * pxPerMs()).toFixed(1)}px`,
                      width: `${((sEnd - sStart) * pxPerMs()).toFixed(1)}px`,
                    }}
                    title="Silence span"
                  />
                )}
              </For>
              <For each={state.transcript().segments}>
                {(seg) => (
                  <Show when={detectSpokenPii(seg.text) !== null}>
                    <div
                      class="tc-tl-pii-pin"
                      style={{
                        left: `${(seg.start_ms * pxPerMs()).toFixed(1)}px`,
                        width: `${Math.max(8, (seg.end_ms - seg.start_ms) * pxPerMs()).toFixed(1)}px`,
                      }}
                      title={
                        state.beepedSegments().has(seg.id)
                          ? "Spoken PII Auto-Beep armed"
                          : "Spoken PII detected"
                      }
                    />
                  </Show>
                )}
              </For>
            </div>

            {/* 4. License Plate Lane */}
            {renderRedactionLane(plateTracks, false)}

            {/* 5. Face Lane */}
            {renderRedactionLane(faceTracks, true)}

            {/* 6. Screen PII / QR Lane */}
            {renderRedactionLane(piiTracks, false)}

            {/* Playhead Needle */}
            <div
              class="tc-playhead"
              style={{ left: `${playheadPx().toFixed(1)}px` }}
            >
              <div class="tc-playhead__head" />
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};
