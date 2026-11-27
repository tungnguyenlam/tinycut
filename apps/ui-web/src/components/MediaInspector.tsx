import { For, Show, createSignal, type Component } from "solid-js";
import type { ModelType, Project } from "../bindings";
import { commands, isTauriRuntime } from "../bridge";
import { formatSegTime } from "../domainExt";
import { newUuid, sampleMedia, useStudio } from "../state";
import { IconLock, IconUpload } from "./Icons";

function formatModelSize(bytes: number): string {
  if (bytes >= 1_000_000) {
    return `${(bytes / 1_000_000).toFixed(1)} MB`;
  }
  if (bytes >= 1_000) {
    return `${Math.round(bytes / 1_000)} KB`;
  }
  return `${bytes} B`;
}

export const MediaInspector: Component = () => {
  const state = useStudio();
  const [pathInput, setPathInput] = createSignal(state.project().media.path);
  const [openAsNda, setOpenAsNda] = createSignal(
    state.project().is_nda_private
  );

  const installedCount = () => state.models().filter((m) => m.installed).length;
  const busy = () => state.busyJob() !== null;

  async function onOpenPath(): Promise<void> {
    const p = pathInput().trim();
    if (!p) {
      state.notify("warning", "Enter a local video or audio path");
      return;
    }
    const nda = openAsNda();
    if (isTauriRuntime()) {
      try {
        const proj = await commands.openProjectFromPath(p, nda);
        state.setProject(proj);
        state.setIsDemoMedia(false);
        state.setCurrentMs(0);
        state.setPlaying(false);
        state.setRecentProjects((rec) => [
          proj,
          ...rec.filter((x) => x.id !== proj.id),
        ]);
        state.hydrateMediaAssets(proj.media);
        state.notify(
          "success",
          `Opened ${proj.title} (${proj.media.width}×${proj.media.height} · ${Math.round(proj.media.fps)}fps)`
        );
        state.setMode("subtitles");
      } catch (e) {
        state.notify("error", `Open failed: ${String(e)}`);
      }
    } else {
      const fname = p.split(/[\\/]/).pop() ?? "project";
      const title = fname.split(".")[0] || "project";
      const proj: Project = {
        id: newUuid(),
        title,
        media: sampleMedia(p),
        is_nda_private: nda,
        inference_mode: "local",
        locale: "vi-VN",
        content_domain: "creator",
      };
      state.setProject(proj);
      state.setRecentProjects((rec) => [proj, ...rec]);
      state.notify("info", `Loaded '${title}' (browser preview)`);
    }
  }

  async function onDownloadAll(): Promise<void> {
    state.setBusyJob("Downloading default models");
    try {
      if (isTauriRuntime()) {
        const updated = await commands.downloadAllDefaultModels();
        state.setModels(updated);
        state.notify("success", "All default lightweight models downloaded");
      } else {
        state.setModels((prev) =>
          prev.map((m) => ({
            ...m,
            installed: true,
            bytes_on_disk: m.spec.size_bytes,
          }))
        );
        state.notify("success", "Preview: marked all default models ready");
      }
    } catch (e) {
      state.notify("error", `Model download failed: ${String(e)}`);
    } finally {
      state.setBusyJob(null);
    }
  }

  async function onDownloadOne(task: ModelType, name: string): Promise<void> {
    state.setBusyJob(`Downloading ${name}`);
    try {
      if (isTauriRuntime()) {
        const updated = await commands.downloadModel(task);
        state.setModels(updated);
        state.notify("success", `Downloaded ${name}`);
      } else {
        state.setModels((prev) =>
          prev.map((m) =>
            m.spec.task === task
              ? { ...m, installed: true, bytes_on_disk: m.spec.size_bytes }
              : m
          )
        );
        state.notify("success", `Preview: ${name} ready`);
      }
    } catch (e) {
      state.notify("error", `Download failed: ${String(e)}`);
    } finally {
      state.setBusyJob(null);
    }
  }

  async function onDeleteOne(task: ModelType, name: string): Promise<void> {
    try {
      if (isTauriRuntime()) {
        const updated = await commands.deleteModel(task);
        state.setModels(updated);
        state.notify("info", `Removed ${name} from local cache`);
      } else {
        state.setModels((prev) =>
          prev.map((m) =>
            m.spec.task === task
              ? { ...m, installed: false, bytes_on_disk: 0 }
              : m
          )
        );
      }
    } catch (e) {
      state.notify("error", `Remove failed: ${String(e)}`);
    }
  }

  return (
    <>
      <div class="tc-insp-header">
        <div class="tc-insp-header__title">
          <span>Media &amp; Projects</span>
        </div>
        <button class="tc-btn tc-btn--sm" onClick={() => state.loadDemo()}>
          Load Demo
        </button>
      </div>

      <div class="tc-insp-body">
        <div class="tc-insp-body--form">
          <div class="tc-dropzone">
            <div class="tc-dropzone__icon">
              <IconUpload />
            </div>
            <div class="tc-dropzone__title">Open Local Video or Audio</div>
            <div class="tc-dropzone__hint">
              MP4, MOV, MKV, WAV, M4A · raw 4K stays local
            </div>
            <div class="tc-flex-row tc-w-full">
              <input
                class="tc-input tc-mono tc-grow"
                placeholder="/path/to/street_vlog_4k.mp4"
                value={pathInput()}
                onFocus={() => state.setEditingText(true)}
                onBlur={() => state.setEditingText(false)}
                onInput={(e) => setPathInput(e.currentTarget.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.isComposing) void onOpenPath();
                }}
              />
              <button class="tc-btn" onClick={() => void onOpenPath()}>
                Open
              </button>
            </div>
            <div class="tc-flex-between tc-w-full">
              <span class="tc-form-row__sub tc-flex-row">
                <IconLock />
                Open with NDA Private Lock
              </span>
              <button
                class={
                  openAsNda()
                    ? "tc-switch tc-switch--warn tc-switch--on"
                    : "tc-switch tc-switch--warn"
                }
                role="switch"
                aria-checked={openAsNda()}
                onClick={() => setOpenAsNda(!openAsNda())}
              />
            </div>
          </div>

          {/* Active Media */}
          <div class="tc-group">
            <div class="tc-group__title">Active Media</div>
            <div class="tc-group__card">
              <div class="tc-form-row">
                <span class="tc-form-row__label">Title</span>
                <span class="tc-form-row__sub">{state.project().title}</span>
              </div>
              <div class="tc-form-row">
                <span class="tc-form-row__label">Resolution</span>
                <span class="tc-form-row__sub tc-mono">
                  {state.project().media.width}×{state.project().media.height} ·{" "}
                  {Math.round(state.project().media.fps)}fps
                </span>
              </div>
              <div class="tc-form-row">
                <span class="tc-form-row__label">Duration</span>
                <span class="tc-form-row__sub tc-mono">
                  {formatSegTime(state.project().media.duration_ms)}
                </span>
              </div>
              <div class="tc-form-row">
                <span class="tc-form-row__label">Locale</span>
                <span class="tc-form-row__sub tc-mono">
                  {state.project().locale}
                </span>
              </div>
            </div>
          </div>

          {/* Local Lightweight AI Models */}
          <div class="tc-group">
            <div class="tc-group__title">
              <span>Default Lightweight Models</span>
              <div class="tc-flex-row">
                <span class="tc-group__sub tc-mono">
                  {installedCount()}/{state.models().length}
                </span>
                <Show when={installedCount() < state.models().length}>
                  <button
                    class="tc-btn tc-btn--sm"
                    disabled={busy()}
                    onClick={() => void onDownloadAll()}
                  >
                    {busy() ? "Downloading…" : "Download All"}
                  </button>
                </Show>
              </div>
            </div>
            <div class="tc-group__card">
              <For each={state.models()}>
                {(model) => {
                  const sizeStr = () =>
                    formatModelSize(
                      model.installed
                        ? model.bytes_on_disk
                        : model.spec.size_bytes
                    );
                  return (
                    <div class="tc-form-row">
                      <div>
                        <div class="tc-form-row__label">
                          {model.spec.task_label}
                        </div>
                        <div class="tc-form-row__sub tc-mono">
                          {model.spec.display_name} · {sizeStr()} ·{" "}
                          {model.spec.license}
                        </div>
                      </div>
                      <Show
                        when={model.installed}
                        fallback={
                          <button
                            class="tc-btn tc-btn--sm"
                            disabled={busy()}
                            onClick={() =>
                              void onDownloadOne(
                                model.spec.task,
                                model.spec.display_name
                              )
                            }
                          >
                            Download
                          </button>
                        }
                      >
                        <button
                          class="tc-btn tc-btn--ghost tc-btn--sm"
                          disabled={busy()}
                          title="Remove cached model file"
                          onClick={() =>
                            void onDeleteOne(
                              model.spec.task,
                              model.spec.display_name
                            )
                          }
                        >
                          Ready
                        </button>
                      </Show>
                    </div>
                  );
                }}
              </For>
              <div class="tc-form-row">
                <div>
                  <div class="tc-form-row__label">QR Code Detection</div>
                  <div class="tc-form-row__sub tc-mono">
                    rqrr (Pure Rust) · 0 MB · MIT/Apache-2.0
                  </div>
                </div>
                <span class="tc-form-row__sub tc-mono">Built-in</span>
              </div>
            </div>
          </div>

          {/* Privacy Firewall */}
          <div class="tc-group">
            <div class="tc-group__title">
              <span>Privacy Firewall</span>
              <Show
                when={state.project().is_nda_private}
                fallback={<span class="tc-group__sub">Local Default</span>}
              >
                <span class="tc-privacy-btn__nda">
                  <IconLock />
                  NDA Locked
                </span>
              </Show>
            </div>
            <div class="tc-group__card">
              <div class="tc-form-row">
                <div>
                  <div class="tc-form-row__label">Anonymized diffs</div>
                  <div class="tc-form-row__sub">
                    PII scrubbed on-device; never sends faces/plates
                  </div>
                </div>
                <button
                  class={
                    state.optInTelemetry() && !state.project().is_nda_private
                      ? "tc-switch tc-switch--on"
                      : "tc-switch"
                  }
                  disabled={state.project().is_nda_private}
                  role="switch"
                  aria-checked={
                    state.optInTelemetry() && !state.project().is_nda_private
                  }
                  onClick={() => {
                    const next = !state.optInTelemetry();
                    state.setOptInTelemetry(next);
                    state.notify(
                      "info",
                      next
                        ? "Opt-in anonymized text diffs enabled"
                        : "Telemetry disabled — 100% offline"
                    );
                  }}
                />
              </div>
            </div>
          </div>

          <div class="tc-group__title">
            <span>Recent Projects</span>
            <span class="tc-group__sub tc-mono">
              {state.recentProjects().length}
            </span>
          </div>
        </div>

        <div class="tc-insp-body--list">
          <For each={state.recentProjects()}>
            {(proj) => {
              const isCur = () => proj.id === state.project().id;
              return (
                <div
                  class={isCur() ? "tc-row tc-row--active" : "tc-row"}
                  onClick={() => {
                    setPathInput(proj.media.path);
                    state.setProject(proj);
                    const isDemo =
                      proj.media.path === "/videos/saigon_street_vlog_4k.mp4";
                    state.setIsDemoMedia(isDemo);
                    if (!isDemo) {
                      state.hydrateMediaAssets(proj.media);
                    }
                    state.notify("info", `Switched to ${proj.title}`);
                  }}
                >
                  <div class="tc-flex-between">
                    <div>
                      <div>{proj.title}</div>
                      <div class="tc-form-row__sub tc-mono">
                        {proj.media.width}×{proj.media.height} ·{" "}
                        {formatSegTime(proj.media.duration_ms)}
                      </div>
                    </div>
                    <Show when={proj.is_nda_private}>
                      <span class="tc-privacy-btn__nda">
                        <IconLock />
                      </span>
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
