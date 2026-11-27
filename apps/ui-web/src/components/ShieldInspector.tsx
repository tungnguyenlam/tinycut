import { For, Show, type Component } from "solid-js";
import type { BlurStyle, RedactionTrack } from "../bindings";
import { commands, isTauriRuntime } from "../bridge";
import { formatSegTime, frameToMs } from "../domainExt";
import { sampleTracks, useStudio } from "../state";

const BLUR_STYLES: [BlurStyle, string][] = [
  ["gaussian", "Gaussian"],
  ["pixelate", "Pixelate"],
  ["solid_mask", "Solid"],
];

export const ShieldInspector: Component = () => {
  const state = useStudio();

  const cfg = () => state.shieldCfg();
  const tracks = () => state.tracks();
  const fps = () => Math.max(1, state.project().media.fps);
  const busy = () => state.busyJob() !== null;

  const plateCount = () =>
    tracks().filter((t) => t.kind === "license_plate").length;
  const faceCount = () => tracks().filter((t) => t.kind === "face").length;
  const piiCount = () =>
    tracks().filter((t) => t.kind === "screen_pii").length;
  const qrCount = () => tracks().filter((t) => t.kind === "qr_code").length;

  async function onScan(): Promise<void> {
    const media = state.project().media;
    const scanCfg = state.shieldCfg();
    state.setBusyJob("Privacy Shield Scan");
    try {
      if (isTauriRuntime()) {
        const detected = await commands.detectPrivacyLocal(media, scanCfg);
        state.pushUndo();
        state.setTracks(detected);
        state.notify("success", `Detected ${detected.length} privacy track(s)`);
      } else {
        state.pushUndo();
        state.setTracks(sampleTracks());
        state.notify(
          "success",
          "Preview: ByteTrack + Zero-Leak updated 4 tracks"
        );
      }
    } catch (e) {
      state.notify("error", `Shield scan failed: ${String(e)}`);
    } finally {
      state.setBusyJob(null);
    }
  }

  function swatchClass(track: RedactionTrack, isRejected: boolean): string {
    if (isRejected) return "tc-swatch tc-swatch--muted";
    if (track.whitelisted) return "tc-swatch tc-swatch--wl";
    switch (track.kind) {
      case "license_plate":
        return "tc-swatch tc-swatch--plate";
      case "face":
        return "tc-swatch tc-swatch--face";
      case "screen_pii":
      case "qr_code":
        return "tc-swatch tc-swatch--pii";
    }
  }

  function kindTitle(track: RedactionTrack): string {
    switch (track.kind) {
      case "license_plate":
        return "License Plate";
      case "face":
        return track.whitelisted ? "Creator Face" : "Bystander Face";
      case "screen_pii":
        return "Screen PII";
      case "qr_code":
        return "QR Code";
    }
  }

  return (
    <>
      <div class="tc-insp-header">
        <div class="tc-insp-header__title">
          <span>Privacy Shield</span>
          <span class="tc-insp-header__count tc-mono">{tracks().length}</span>
        </div>
        <button
          class="tc-btn tc-btn--sm"
          disabled={busy()}
          onClick={() => void onScan()}
        >
          {busy() ? "Scanning…" : "Detect"}
        </button>
      </div>

      <div class="tc-insp-body">
        <div class="tc-insp-body--form">
          {/* 1. Detectors Group */}
          <div class="tc-group">
            <div class="tc-group__title">
              <span>Detectors</span>
              <span class="tc-group__sub">Local ONNX</span>
            </div>
            <div class="tc-group__card">
              <div class="tc-form-row">
                <div class="tc-form-row__label">
                  <span class="tc-swatch tc-swatch--plate" />
                  <span>License plates</span>
                  <span class="tc-form-row__sub tc-mono">{plateCount()}</span>
                </div>
                <button
                  class={
                    cfg().detect_plates
                      ? "tc-switch tc-switch--on"
                      : "tc-switch"
                  }
                  role="switch"
                  aria-checked={cfg().detect_plates}
                  onClick={() =>
                    state.setShieldCfg((c) => ({
                      ...c,
                      detect_plates: !c.detect_plates,
                    }))
                  }
                />
              </div>

              <div class="tc-form-row">
                <div class="tc-form-row__label">
                  <span class="tc-swatch tc-swatch--face" />
                  <span>Faces</span>
                  <span class="tc-form-row__sub tc-mono">{faceCount()}</span>
                </div>
                <button
                  class={
                    cfg().detect_faces ? "tc-switch tc-switch--on" : "tc-switch"
                  }
                  role="switch"
                  aria-checked={cfg().detect_faces}
                  onClick={() =>
                    state.setShieldCfg((c) => ({
                      ...c,
                      detect_faces: !c.detect_faces,
                    }))
                  }
                />
              </div>

              <div class="tc-form-row">
                <div class="tc-form-row__label">
                  <span class="tc-swatch tc-swatch--pii" />
                  <span>Screen PII</span>
                  <span class="tc-form-row__sub tc-mono">{piiCount()}</span>
                </div>
                <button
                  class={
                    cfg().detect_screen_pii
                      ? "tc-switch tc-switch--on"
                      : "tc-switch"
                  }
                  role="switch"
                  aria-checked={cfg().detect_screen_pii}
                  onClick={() =>
                    state.setShieldCfg((c) => ({
                      ...c,
                      detect_screen_pii: !c.detect_screen_pii,
                    }))
                  }
                />
              </div>

              <div class="tc-form-row">
                <div class="tc-form-row__label">
                  <span class="tc-swatch tc-swatch--pii" />
                  <span>QR codes</span>
                  <span class="tc-form-row__sub tc-mono">{qrCount()}</span>
                </div>
                <button
                  class={
                    cfg().detect_qr ? "tc-switch tc-switch--on" : "tc-switch"
                  }
                  role="switch"
                  aria-checked={cfg().detect_qr}
                  onClick={() =>
                    state.setShieldCfg((c) => ({
                      ...c,
                      detect_qr: !c.detect_qr,
                    }))
                  }
                />
              </div>
            </div>
          </div>

          {/* 2. Mask Style & Zero-Leak Padding Group */}
          <div class="tc-group">
            <div class="tc-group__title">
              <span>Redaction &amp; Zero-Leak</span>
              <span class="tc-group__sub">Live canvas + FFmpeg</span>
            </div>
            <div class="tc-group__card">
              <div class="tc-form-row tc-form-row--stack">
                <div class="tc-seg">
                  <For each={BLUR_STYLES}>
                    {([styleOpt, label]) => (
                      <button
                        class={
                          cfg().blur_style === styleOpt
                            ? "tc-seg__btn tc-seg__btn--active"
                            : "tc-seg__btn"
                        }
                        onClick={() =>
                          state.setShieldCfg((c) => ({
                            ...c,
                            blur_style: styleOpt,
                          }))
                        }
                      >
                        {label}
                      </button>
                    )}
                  </For>
                </div>
              </div>

              <div class="tc-form-row">
                <span class="tc-form-row__label">Spatial box margin</span>
                <div class="tc-stepper">
                  <button
                    class="tc-stepper__btn"
                    onClick={() =>
                      state.setShieldCfg((c) => ({
                        ...c,
                        spatial_padding_ratio: Math.max(
                          0,
                          Number((c.spatial_padding_ratio - 0.05).toFixed(2))
                        ),
                      }))
                    }
                  >
                    −
                  </button>
                  <span class="tc-stepper__val tc-mono">
                    +{Math.round(cfg().spatial_padding_ratio * 100)}%
                  </span>
                  <button
                    class="tc-stepper__btn"
                    onClick={() =>
                      state.setShieldCfg((c) => ({
                        ...c,
                        spatial_padding_ratio: Math.min(
                          0.3,
                          Number((c.spatial_padding_ratio + 0.05).toFixed(2))
                        ),
                      }))
                    }
                  >
                    +
                  </button>
                </div>
              </div>

              <div class="tc-form-row">
                <span class="tc-form-row__label">Temporal lead / trail</span>
                <div class="tc-stepper">
                  <button
                    class="tc-stepper__btn"
                    onClick={() =>
                      state.setShieldCfg((c) => ({
                        ...c,
                        temporal_padding_frames: Math.max(
                          0,
                          c.temporal_padding_frames - 1
                        ),
                      }))
                    }
                  >
                    −
                  </button>
                  <span class="tc-stepper__val tc-mono">
                    ±{cfg().temporal_padding_frames}f
                  </span>
                  <button
                    class="tc-stepper__btn"
                    onClick={() =>
                      state.setShieldCfg((c) => ({
                        ...c,
                        temporal_padding_frames: Math.min(
                          15,
                          c.temporal_padding_frames + 1
                        ),
                      }))
                    }
                  >
                    +
                  </button>
                </div>
              </div>
            </div>
          </div>

          <div class="tc-group__title">
            <span>Tracked Objects</span>
            <span class="tc-group__sub">Click row to seek</span>
          </div>
        </div>

        {/* 3. Flat Track List Rows */}
        <div class="tc-insp-body--list">
          <For each={tracks()}>
            {(track) => {
              const tid = track.track_id;
              const isSelected = () => state.selectedTrackId() === tid;
              const isRejected = () => state.trackVerdicts()[tid] === "not_pii";
              const firstMs = () =>
                track.keyframes[0]
                  ? frameToMs(track.keyframes[0].frame_index, fps())
                  : 0;
              const lastMs = () =>
                track.keyframes.length > 0
                  ? frameToMs(
                      track.keyframes[track.keyframes.length - 1].frame_index,
                      fps()
                    )
                  : firstMs();
              const confPct = () =>
                Math.round((track.keyframes[0]?.confidence ?? 0.9) * 100);
              const isBlurring = () =>
                track.enabled && !track.whitelisted && !isRejected();

              return (
                <div
                  class={isSelected() ? "tc-row tc-row--active" : "tc-row"}
                  onClick={() => {
                    state.setSelectedTrackId(tid);
                    state.seekMs(firstMs());
                  }}
                >
                  <div class="tc-track-row">
                    <div class="tc-track-row__top">
                      <div class="tc-track-row__title">
                        <span class={swatchClass(track, isRejected())} />
                        <span>
                          {kindTitle(track)} #{tid}
                        </span>
                        <span class="tc-track-row__meta tc-mono">
                          {confPct()}%
                        </span>
                      </div>
                      <span class="tc-track-row__meta tc-mono">
                        {formatSegTime(firstMs())} – {formatSegTime(lastMs())}
                      </span>
                    </div>

                    <Show when={isSelected()}>
                      <div class="tc-track-row__controls">
                        <button
                          class={
                            isBlurring()
                              ? "tc-btn tc-btn--sm tc-btn--active"
                              : "tc-btn tc-btn--sm"
                          }
                          onClick={(e) => {
                            e.stopPropagation();
                            state.pushUndo();
                            let nowEnabled = false;
                            state.setTracks((prev) =>
                              prev.map((t) => {
                                if (t.track_id === tid) {
                                  nowEnabled = !t.enabled;
                                  return {
                                    ...t,
                                    enabled: nowEnabled,
                                    whitelisted: nowEnabled
                                      ? false
                                      : t.whitelisted,
                                  };
                                }
                                return t;
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
                          {isBlurring() ? "Blurring" : "Enable blur"}
                        </button>

                        <Show when={track.kind === "face"}>
                          <button
                            class={
                              track.whitelisted
                                ? "tc-btn tc-btn--sm tc-btn--active"
                                : "tc-btn tc-btn--sm"
                            }
                            onClick={(e) => {
                              e.stopPropagation();
                              state.pushUndo();
                              let becomingWl = false;
                              state.setTracks((prev) =>
                                prev.map((t) => {
                                  if (t.track_id === tid) {
                                    becomingWl = !t.whitelisted;
                                    return {
                                      ...t,
                                      whitelisted: becomingWl,
                                      enabled: !becomingWl,
                                    };
                                  }
                                  return t;
                                })
                              );
                              if (becomingWl) {
                                state.setTrackVerdicts((v) => ({
                                  ...v,
                                  [tid]: "whitelisted",
                                }));
                                state.notify(
                                  "success",
                                  `Saved Face #${tid} to Creator Whitelist`
                                );
                                if (isTauriRuntime()) {
                                  void commands
                                    .whitelistCreatorFace(
                                      `creator-${tid}`,
                                      "Host",
                                      [0.6, 0.8]
                                    )
                                    .catch(() => {});
                                }
                              } else {
                                state.setTrackVerdicts((v) => {
                                  const next = { ...v };
                                  delete next[tid];
                                  return next;
                                });
                                state.notify(
                                  "info",
                                  `Un-whitelisted Face #${tid}`
                                );
                              }
                            }}
                          >
                            {track.whitelisted ? "Whitelisted" : "Keep Unblurred"}
                          </button>
                        </Show>

                        <button
                          class={
                            isRejected()
                              ? "tc-btn tc-btn--sm tc-btn--danger"
                              : "tc-btn tc-btn--ghost tc-btn--sm"
                          }
                          onClick={(e) => {
                            e.stopPropagation();
                            state.pushUndo();
                            if (isRejected()) {
                              state.setTrackVerdicts((v) => {
                                const next = { ...v };
                                delete next[tid];
                                return next;
                              });
                              state.setTracks((prev) =>
                                prev.map((t) =>
                                  t.track_id === tid
                                    ? { ...t, enabled: true }
                                    : t
                                )
                              );
                              state.notify("info", `Restored Track #${tid}`);
                            } else {
                              state.setTrackVerdicts((v) => ({
                                ...v,
                                [tid]: "not_pii",
                              }));
                              state.setTracks((prev) =>
                                prev.map((t) =>
                                  t.track_id === tid
                                    ? { ...t, enabled: false, whitelisted: false }
                                    : t
                                )
                              );
                              state.notify("info", `Marked Track #${tid} Not PII`);
                            }
                          }}
                        >
                          {isRejected() ? "Restore" : "Not PII"}
                        </button>
                      </div>
                    </Show>
                  </div>
                </div>
              );
            }}
          </For>
        </div>
      </div>
    </>
  );
};
