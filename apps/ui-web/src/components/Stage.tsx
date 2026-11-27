import {
  For,
  Show,
  createEffect,
  createMemo,
  onCleanup,
  onMount,
  type Component,
} from "solid-js";
import type { RedactionTrack } from "../bindings";
import { commands, convertFileSrc, isTauriRuntime } from "../bridge";
import {
  formatSmpte,
  interpolateTrackAt,
  msToFrame,
  type InterpolatedBox,
} from "../domainExt";
import { useStudio } from "../state";
import {
  IconChevronDown,
  IconLock,
  IconPause,
  IconPlay,
  IconSkipBack,
  IconSliders,
  IconStepBack,
  IconStepForward,
} from "./Icons";

interface RenderedTrackHit {
  track: RedactionTrack;
  idx: number;
  interp: InterpolatedBox;
}

function trackColor(track: RedactionTrack, isRejected: boolean): string {
  if (isRejected || (!track.enabled && !track.whitelisted)) {
    return "rgba(180, 186, 198, 0.75)";
  }
  if (track.whitelisted) return "#34d399";
  switch (track.kind) {
    case "license_plate":
      return "#f59e0b";
    case "face":
      return "#f43f5e";
    case "screen_pii":
    case "qr_code":
      return "#a855f7";
  }
}

function trackKindLabel(track: RedactionTrack): string {
  switch (track.kind) {
    case "license_plate":
      return "Plate";
    case "face":
      return track.whitelisted ? "Creator" : "Face";
    case "screen_pii":
      return "Screen PII";
    case "qr_code":
      return "QR";
  }
}

export const Stage: Component = () => {
  const state = useStudio();
  let videoEl: HTMLVideoElement | undefined;
  let canvasEl: HTMLCanvasElement | undefined;
  let viewportEl: HTMLDivElement | undefined;
  let lastHits: RenderedTrackHit[] = [];

  const fps = () => Math.max(1, state.project().media.fps);
  const currentFrame = () => msToFrame(state.currentMs(), fps());

  const videoSrc = createMemo(() => {
    if (state.isDemoMedia()) return null;
    const rawPath = state.previewProxyPath() ?? state.project().media.path;
    return convertFileSrc(rawPath);
  });

  const activeSeg = createMemo(() => {
    const ms = state.currentMs();
    return (
      state
        .transcript()
        .segments.find((s) => ms >= s.start_ms && ms <= s.end_ms) ?? null
    );
  });

  const isActiveBeeped = () => {
    const seg = activeSeg();
    return seg ? state.beepedSegments().has(seg.id) : false;
  };

  // Bidirectional sync: StudioState -> HTML <video>
  createEffect(() => {
    const src = videoSrc();
    const playing = state.playing();
    const rate = state.playbackRate();
    if (!src || !videoEl) return;

    videoEl.playbackRate = rate;
    if (playing) {
      if (videoEl.paused) {
        void videoEl.play().catch(() => {
          state.setPlaying(false);
        });
      }
    } else if (!videoEl.paused) {
      videoEl.pause();
    }
  });

  createEffect(() => {
    const src = videoSrc();
    const ms = state.currentMs();
    if (!src || !videoEl) return;
    const videoMs = videoEl.currentTime * 1000;
    if (Math.abs(videoMs - ms) > 85) {
      videoEl.currentTime = ms / 1000;
    }
  });

  // High-performance 60fps <canvas> redaction overlay loop (no whole-tree DOM churn)
  function drawRedactionCanvas(): void {
    const canvas = canvasEl;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return;

    const dpr = window.devicePixelRatio || 1;
    const targetW = Math.round(rect.width * dpr);
    const targetH = Math.round(rect.height * dpr);
    if (canvas.width !== targetW || canvas.height !== targetH) {
      canvas.width = targetW;
      canvas.height = targetH;
    }

    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.save();
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, rect.width, rect.height);

    const frame = msToFrame(state.currentMs(), Math.max(1, state.project().media.fps));
    const cfg = state.shieldCfg();
    const tracks = state.tracks();
    const verdicts = state.trackVerdicts();
    const selectedId = state.selectedTrackId();
    const showMasks = state.showMasks();
    const showMargin = state.showZeroLeakMargin();
    const hasLiveVideo =
      !state.isDemoMedia() &&
      videoEl !== undefined &&
      videoEl.readyState >= 2 &&
      videoEl.videoWidth > 0;

    const hits: RenderedTrackHit[] = [];

    for (let idx = 0; idx < tracks.length; idx++) {
      const track = tracks[idx];
      const interp = interpolateTrackAt(track, frame, cfg);
      if (!interp) continue;
      hits.push({ track, idx, interp });

      const p = interp.padded;
      const r = interp.raw;
      const x = p.x * rect.width;
      const y = p.y * rect.height;
      const w = p.w * rect.width;
      const h = p.h * rect.height;

      const isRejected = verdicts[track.track_id] === "not_pii";
      const isSelected = selectedId === track.track_id;
      const color = trackColor(track, isRejected);
      const shouldMask =
        showMasks && track.enabled && !track.whitelisted && !isRejected;

      // 1. Mask Fill (Gaussian / Pixelate / Solid)
      if (shouldMask && w > 1 && h > 1) {
        ctx.save();
        ctx.beginPath();
        ctx.roundRect(x, y, w, h, 4);
        ctx.clip();

        if (cfg.blur_style === "solid_mask") {
          ctx.fillStyle = "#0b0c0e";
          ctx.fillRect(x, y, w, h);
        } else if (cfg.blur_style === "pixelate") {
          const cell = Math.max(6, Math.round(Math.min(w, h) / 6));
          if (hasLiveVideo && videoEl) {
            ctx.imageSmoothingEnabled = false;
            const sx = p.x * videoEl.videoWidth;
            const sy = p.y * videoEl.videoHeight;
            const sw = Math.max(1, p.w * videoEl.videoWidth);
            const sh = Math.max(1, p.h * videoEl.videoHeight);
            const tmpW = Math.max(2, Math.round(w / cell));
            const tmpH = Math.max(2, Math.round(h / cell));
            const off = document.createElement("canvas");
            off.width = tmpW;
            off.height = tmpH;
            const octx = off.getContext("2d");
            if (octx) {
              octx.drawImage(videoEl, sx, sy, sw, sh, 0, 0, tmpW, tmpH);
              ctx.drawImage(off, 0, 0, tmpW, tmpH, x, y, w, h);
            }
            ctx.imageSmoothingEnabled = true;
          } else {
            for (let py = y; py < y + h; py += cell) {
              for (let px = x; px < x + w; px += cell) {
                const odd =
                  (Math.floor((px - x) / cell) + Math.floor((py - y) / cell)) % 2 === 0;
                ctx.fillStyle = odd
                  ? "rgba(22, 26, 36, 0.88)"
                  : "rgba(38, 44, 58, 0.88)";
                ctx.fillRect(px, py, cell, cell);
              }
            }
          }
        } else {
          // Gaussian blur preview
          if (hasLiveVideo && videoEl) {
            ctx.filter = "blur(12px)";
            ctx.drawImage(
              videoEl,
              p.x * videoEl.videoWidth,
              p.y * videoEl.videoHeight,
              Math.max(1, p.w * videoEl.videoWidth),
              Math.max(1, p.h * videoEl.videoHeight),
              x - 10,
              y - 10,
              w + 20,
              h + 20
            );
            ctx.filter = "none";
          } else {
            ctx.fillStyle = "rgba(18, 22, 30, 0.72)";
            ctx.fillRect(x, y, w, h);
            const grad = ctx.createRadialGradient(
              x + w * 0.5,
              y + h * 0.5,
              2,
              x + w * 0.5,
              y + h * 0.5,
              Math.max(w, h) * 0.6
            );
            grad.addColorStop(0, "rgba(255, 255, 255, 0.16)");
            grad.addColorStop(1, "rgba(10, 12, 18, 0.35)");
            ctx.fillStyle = grad;
            ctx.fillRect(x, y, w, h);
          }
        }
        ctx.restore();
      }

      // 2. Zero-Leak inner raw detection box (+10% margin visualization)
      if (showMargin && cfg.spatial_padding_ratio > 0.001 && !isRejected) {
        ctx.save();
        ctx.strokeStyle = "rgba(255, 255, 255, 0.45)";
        ctx.lineWidth = 1;
        ctx.setLineDash([3, 3]);
        ctx.strokeRect(
          r.x * rect.width,
          r.y * rect.height,
          r.w * rect.width,
          r.h * rect.height
        );
        ctx.restore();
      }

      // 3. Outer Padded Detection Box Border
      ctx.save();
      ctx.strokeStyle = color;
      ctx.lineWidth = isSelected ? 2 : 1.5;
      if (track.whitelisted) {
        ctx.setLineDash([5, 4]);
      } else if (isRejected || (!track.enabled && !track.whitelisted)) {
        ctx.setLineDash([2, 3]);
        ctx.globalAlpha = 0.55;
      }
      ctx.strokeRect(x, y, w, h);
      ctx.restore();

      // 4. Corner Handles for Selected Track
      if (isSelected) {
        ctx.save();
        ctx.fillStyle = "#ffffff";
        ctx.strokeStyle = "rgba(0,0,0,0.45)";
        ctx.lineWidth = 1;
        const hs = 7;
        const corners = [
          [x - hs / 2, y - hs / 2],
          [x + w - hs / 2, y - hs / 2],
          [x - hs / 2, y + h - hs / 2],
          [x + w - hs / 2, y + h - hs / 2],
        ];
        for (const [cx, cy] of corners) {
          ctx.fillRect(cx, cy, hs, hs);
          ctx.strokeRect(cx, cy, hs, hs);
        }
        ctx.restore();
      }

      // 5. Track Label Tag
      const tagText = `${trackKindLabel(track)} #${track.track_id}`;
      ctx.save();
      ctx.font = '600 10px ui-monospace, SFMono-Regular, Menlo, monospace';
      const tm = ctx.measureText(tagText);
      const tagW = tm.width + 10;
      const tagH = 16;
      const tagY = y >= tagH + 4 ? y - tagH - 3 : y + 3;
      ctx.fillStyle = color;
      ctx.beginPath();
      ctx.roundRect(x, tagY, tagW, tagH, 4);
      ctx.fill();
      ctx.fillStyle = track.kind === "face" && !track.whitelisted && !isRejected ? "#ffffff" : "#0b0d10";
      ctx.fillText(tagText, x + 5, tagY + 11.5);
      ctx.restore();
    }

    lastHits = hits;
    ctx.restore();
  }

  onMount(() => {
    let rafId = 0;
    const tick = () => {
      // If real video is playing, pull its smooth timestamp into state
      if (
        !state.isDemoMedia() &&
        videoEl &&
        !videoEl.paused &&
        !videoEl.ended
      ) {
        const ms = Math.round(videoEl.currentTime * 1000);
        if (Math.abs(ms - state.currentMs()) >= 8) {
          state.setCurrentMs(ms);
        }
      }
      drawRedactionCanvas();
      rafId = window.requestAnimationFrame(tick);
    };
    rafId = window.requestAnimationFrame(tick);

    const ro = new ResizeObserver(() => drawRedactionCanvas());
    if (viewportEl) ro.observe(viewportEl);

    onCleanup(() => {
      window.cancelAnimationFrame(rafId);
      ro.disconnect();
    });
  });

  function hitTestTrack(clientX: number, clientY: number): RenderedTrackHit | null {
    const canvas = canvasEl;
    if (!canvas) return null;
    const rect = canvas.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return null;
    const nx = (clientX - rect.left) / rect.width;
    const ny = (clientY - rect.top) / rect.height;

    // Check in reverse paint order so topmost boxes win
    for (let i = lastHits.length - 1; i >= 0; i--) {
      const h = lastHits[i];
      const b = h.interp.padded;
      if (nx >= b.x && nx <= b.x + b.w && ny >= b.y && ny <= b.y + b.h) {
        return h;
      }
    }
    return null;
  }

  function toggleWhitelist(trackId: number): void {
    state.pushUndo();
    let becomingWl = false;
    state.setTracks((prev) =>
      prev.map((t) => {
        if (t.track_id === trackId && t.kind === "face") {
          becomingWl = !t.whitelisted;
          return { ...t, whitelisted: becomingWl, enabled: !becomingWl };
        }
        return t;
      })
    );
    if (becomingWl) {
      state.setTrackVerdicts((v) => ({ ...v, [trackId]: "whitelisted" }));
      state.notify("success", `Face #${trackId} saved to Creator Whitelist`);
      if (isTauriRuntime()) {
        void commands
          .whitelistCreatorFace(`creator-${trackId}`, "Host", [0.6, 0.8])
          .catch(() => {});
      }
    } else {
      state.setTrackVerdicts((v) => {
        const next = { ...v };
        delete next[trackId];
        return next;
      });
      state.notify("info", `Removed Face #${trackId} from Creator Whitelist`);
    }
  }

  const selectedVisibleTrack = createMemo(() => {
    const tid = state.selectedTrackId();
    if (tid === null) return null;
    const track = state.tracks().find((t) => t.track_id === tid);
    if (!track) return null;
    const interp = interpolateTrackAt(track, currentFrame(), state.shieldCfg());
    if (!interp) return null;
    return { track, interp };
  });

  const demoSubjects = createMemo(() => {
    if (!state.isDemoMedia()) return [];
    const frame = currentFrame();
    const cfg = state.shieldCfg();
    return state
      .tracks()
      .map((track) => {
        const interp = interpolateTrackAt(track, frame, cfg);
        return interp ? { track, interp } : null;
      })
      .filter((x): x is { track: RedactionTrack; interp: InterpolatedBox } => x !== null);
  });

  return (
    <section class="tc-stage-wrap">
      <div class="tc-stage-canvas-area">
        {/* Hover-revealed Minimal Stage HUD */}
        <div
          class={
            state.showViewMenu()
              ? "tc-stage-hud tc-stage-hud--pinned"
              : "tc-stage-hud tc-stage-hud--hoverable"
          }
        >
          <div class="tc-stage-hud__group">
            <Show when={state.isDemoMedia()}>
              <span class="tc-hud-chip">Demo</span>
            </Show>
            <Show when={state.project().is_nda_private}>
              <span class="tc-hud-chip tc-hud-chip--nda">
                <IconLock />
                NDA
              </span>
            </Show>
          </div>

          <div class="tc-stage-hud__group">
            <button
              class="tc-hud-chip tc-hud-chip--btn"
              title="Viewer overlay options"
              onClick={() => {
                const next = !state.showViewMenu();
                state.closeOverlays();
                state.setShowViewMenu(next);
              }}
            >
              <IconSliders />
              View
              <IconChevronDown />
            </button>
          </div>
        </div>

        {/* View Options Glass Popover */}
        <Show when={state.showViewMenu()}>
          <div
            class="tc-click-away"
            onClick={() => state.setShowViewMenu(false)}
          />
          <div class="tc-popover tc-popover--view tc-glass-thick">
            <div class="tc-popover__heading">Stage Overlays</div>
            <div class="tc-form-row">
              <span class="tc-form-row__label">Live blur masks</span>
              <button
                class={state.showMasks() ? "tc-switch tc-switch--on" : "tc-switch"}
                role="switch"
                aria-checked={state.showMasks()}
                onClick={() => state.setShowMasks(!state.showMasks())}
              />
            </div>
            <div class="tc-form-row">
              <span class="tc-form-row__label">Zero-Leak margin</span>
              <button
                class={
                  state.showZeroLeakMargin()
                    ? "tc-switch tc-switch--on"
                    : "tc-switch"
                }
                role="switch"
                aria-checked={state.showZeroLeakMargin()}
                onClick={() =>
                  state.setShowZeroLeakMargin(!state.showZeroLeakMargin())
                }
              />
            </div>
            <div class="tc-form-row">
              <span class="tc-form-row__label">Subtitle preview</span>
              <button
                class={
                  state.showSubtitlesOverlay()
                    ? "tc-switch tc-switch--on"
                    : "tc-switch"
                }
                role="switch"
                aria-checked={state.showSubtitlesOverlay()}
                onClick={() =>
                  state.setShowSubtitlesOverlay(!state.showSubtitlesOverlay())
                }
              />
            </div>
          </div>
        </Show>

        {/* 16:9 Cinema Frame */}
        <div class="tc-stage-viewport" ref={viewportEl}>
          <Show
            when={videoSrc()}
            fallback={
              <div class="tc-demo-scene">
                <div class="tc-demo-scene__sky-glow" />
                <div class="tc-demo-scene__city" />
                <div class="tc-demo-scene__road">
                  <div class="tc-demo-scene__lane-dash" />
                </div>

                <For each={demoSubjects()}>
                  {({ track, interp }) => {
                    const style = () => ({
                      left: `${(interp.raw.x * 100).toFixed(2)}%`,
                      top: `${(interp.raw.y * 100).toFixed(2)}%`,
                      width: `${(interp.raw.w * 100).toFixed(2)}%`,
                      height: `${(interp.raw.h * 100).toFixed(2)}%`,
                    });

                    if (track.kind === "license_plate") {
                      return (
                        <div
                          class="tc-demo-subject tc-demo-subject--plate"
                          style={style()}
                        >
                          <span>59-X3</span>
                          <span>829.41</span>
                        </div>
                      );
                    }
                    if (track.kind === "face" && track.whitelisted) {
                      return (
                        <div
                          class="tc-demo-subject tc-demo-subject--creator"
                          style={style()}
                        >
                          <span class="tc-demo-subject__cap">HOST</span>
                        </div>
                      );
                    }
                    if (track.kind === "face") {
                      return (
                        <div
                          class="tc-demo-subject tc-demo-subject--bystander"
                          style={style()}
                        >
                          <span class="tc-demo-subject__cap">PASSERBY</span>
                        </div>
                      );
                    }
                    return (
                      <div
                        class="tc-demo-subject tc-demo-subject--pii"
                        style={style()}
                      >
                        <span class="tc-demo-subject__sub">
                          Notification · Zalo
                        </span>
                        <span>OTP: 482910 · minh@tinycut.dev</span>
                      </div>
                    );
                  }}
                </For>
              </div>
            }
          >
            {(src) => (
              <video
                ref={videoEl}
                class="tc-video-el"
                src={src()}
                preload="auto"
                playsinline
                onLoadedMetadata={(e) => {
                  const v = e.currentTarget;
                  if (Number.isFinite(v.duration) && v.duration > 0) {
                    const durMs = Math.round(v.duration * 1000);
                    if (Math.abs(durMs - state.project().media.duration_ms) > 500) {
                      state.setProject((p) => ({
                        ...p,
                        media: { ...p.media, duration_ms: durMs },
                      }));
                    }
                  }
                  v.currentTime = state.currentMs() / 1000;
                }}
                onEnded={() => state.setPlaying(false)}
              />
            )}
          </Show>

          {/* 60fps <canvas> Redaction Overlay */}
          <canvas
            ref={canvasEl}
            class="tc-redaction-canvas"
            onClick={(e) => {
              const hit = hitTestTrack(e.clientX, e.clientY);
              if (hit) {
                state.setSelectedTrackId(hit.track.track_id);
                state.setMode("shield");
              } else {
                state.setSelectedTrackId(null);
              }
            }}
            onDblClick={(e) => {
              const hit = hitTestTrack(e.clientX, e.clientY);
              if (hit && hit.track.kind === "face") {
                toggleWhitelist(hit.track.track_id);
              }
            }}
          />

          {/* Floating Quick-Action Pill for Selected Track */}
          <Show when={selectedVisibleTrack()}>
            {(sel) => {
              const p = () => sel().interp.padded;
              const t = () => sel().track;
              const confPct = () => Math.round(sel().interp.confidence * 100);
              return (
                <div
                  class="tc-rbox-selected-bar"
                  style={{
                    left: `${((p().x + p().w * 0.5) * 100).toFixed(2)}%`,
                    top: `${(Math.min(0.92, p().y + p().h) * 100).toFixed(2)}%`,
                  }}
                  onClick={(e) => e.stopPropagation()}
                >
                  <span class="tc-rbox__pill-conf tc-mono">{confPct()}%</span>
                  <Show when={t().kind === "face"}>
                    <button
                      class="tc-rbox__pill-btn tc-rbox__pill-btn--wl"
                      onClick={() => toggleWhitelist(t().track_id)}
                    >
                      {t().whitelisted ? "Un-whitelist" : "Keep Creator"}
                    </button>
                  </Show>
                  <button
                    class="tc-rbox__pill-btn tc-rbox__pill-btn--rej"
                    onClick={() => {
                      const tid = t().track_id;
                      state.pushUndo();
                      state.setTracks((prev) =>
                        prev.map((tr) =>
                          tr.track_id === tid
                            ? { ...tr, enabled: false, whitelisted: false }
                            : tr
                        )
                      );
                      state.setTrackVerdicts((v) => ({ ...v, [tid]: "not_pii" }));
                      state.notify("info", `Track #${tid} marked Not PII`);
                    }}
                  >
                    Not PII
                  </button>
                  <button
                    class="tc-rbox__pill-btn"
                    onClick={() => {
                      const tid = t().track_id;
                      state.pushUndo();
                      let nowEnabled = false;
                      state.setTracks((prev) =>
                        prev.map((tr) => {
                          if (tr.track_id === tid) {
                            nowEnabled = !tr.enabled;
                            return {
                              ...tr,
                              enabled: nowEnabled,
                              whitelisted: nowEnabled ? false : tr.whitelisted,
                            };
                          }
                          return tr;
                        })
                      );
                      if (nowEnabled) {
                        state.setTrackVerdicts((v) => {
                          const next = { ...v };
                          delete next[tid];
                          return next;
                        });
                      }
                    }}
                  >
                    {t().enabled ? "Disable" : "Blur"}
                  </button>
                </div>
              );
            }}
          </Show>

          {/* Burn-In Subtitle Preview */}
          <Show when={state.showSubtitlesOverlay() && activeSeg()}>
            {(seg) => (
              <div class="tc-sub-overlay">
                <Show when={seg().speaker}>
                  {(sp) => <span class="tc-sub-overlay__speaker">{sp()}</span>}
                </Show>
                <div class="tc-sub-overlay__box">
                  <Show when={isActiveBeeped()}>
                    <span class="tc-sub-overlay__beep">[BEEP]</span>
                  </Show>
                  {seg().text}
                </div>
              </div>
            )}
          </Show>
        </div>

        {/* Floating Liquid Glass Transport Capsule */}
        <div class="tc-transport">
          <div class="tc-transport__group">
            <button
              class="tc-transport__icon"
              title="Jump to start"
              onClick={() => state.seekMs(0)}
            >
              <IconSkipBack />
            </button>
            <button
              class="tc-transport__icon"
              title="Step back 1 frame"
              onClick={() => {
                const step = Math.max(16, Math.round(1000 / fps()));
                state.seekMs(Math.max(0, state.currentMs() - step));
              }}
            >
              <IconStepBack />
            </button>
            <button
              class="tc-transport__play"
              title="Play / Pause (Space)"
              onClick={() => state.togglePlay()}
            >
              <Show when={state.playing()} fallback={<IconPlay />}>
                <IconPause />
              </Show>
            </button>
            <button
              class="tc-transport__icon"
              title="Step forward 1 frame"
              onClick={() => {
                const step = Math.max(16, Math.round(1000 / fps()));
                state.seekMs(state.currentMs() + step);
              }}
            >
              <IconStepForward />
            </button>
          </div>

          <div class="tc-transport__tc tc-mono">
            <span>{formatSmpte(state.currentMs(), fps())}</span>
            <span class="tc-transport__tc-total">
              {" "}
              / {formatSmpte(state.project().media.duration_ms, fps())}
            </span>
          </div>

          <div class="tc-transport__group tc-transport__group--end">
            <button
              class="tc-transport__rate tc-mono"
              title="Cycle playback speed (J / K / L)"
              onClick={() => {
                const r = state.playbackRate();
                const next =
                  Math.abs(r - 1.0) < 0.05
                    ? 1.5
                    : Math.abs(r - 1.5) < 0.05
                      ? 2.0
                      : 1.0;
                state.setPlaybackRate(next);
              }}
            >
              {state.playbackRate().toFixed(1)}×
            </button>
          </div>
        </div>
      </div>
    </section>
  );
};
