import {
  For,
  Show,
  createEffect,
  createMemo,
  type Component,
} from "solid-js";
import type { EditDiff, TranscriptSegment } from "../bindings";
import { commands, isTauriRuntime } from "../bridge";
import {
  computeThreeWayDiff,
  countFillersInText,
  detectSpokenPii,
  formatSegTime,
  stripFillersText,
} from "../domainExt";
import { newUuid, sampleTranscript, useStudio } from "../state";
import {
  IconClose,
  IconMerge,
  IconScissors,
  IconSearch,
  IconSparkle,
  IconVolumeOff,
} from "./Icons";

/**
 * Vietnamese-IME-safe textarea (Telex/VNI/IBus/macOS IME).
 * Never overwrites `el.value` while `isComposing` is active, and defers
 * reactive transcript updates until composition commits.
 */
const ImeSafeTextarea: Component<{
  value: string;
  onFocus: () => void;
  onBlur: (finalValue: string) => void;
  onCommit: (nextValue: string) => void;
}> = (props) => {
  let el: HTMLTextAreaElement | undefined;
  let composing = false;

  createEffect(() => {
    const v = props.value;
    if (el && !composing && el.value !== v) {
      el.value = v;
    }
  });

  return (
    <textarea
      ref={el}
      class="tc-textarea"
      rows={2}
      value={props.value}
      onClick={(e) => e.stopPropagation()}
      onFocus={() => props.onFocus()}
      onCompositionStart={() => {
        composing = true;
      }}
      onCompositionEnd={(e) => {
        composing = false;
        props.onCommit(e.currentTarget.value);
      }}
      onInput={(e) => {
        if (!composing && !e.isComposing) {
          props.onCommit(e.currentTarget.value);
        }
      }}
      onBlur={(e) => {
        composing = false;
        props.onBlur(e.currentTarget.value);
      }}
    />
  );
};

export const SubtitlesInspector: Component = () => {
  const state = useStudio();

  const segments = () => state.transcript().segments;
  const busy = () => state.busyJob() !== null;
  const filterQ = () => state.subFilter().trim().toLowerCase();

  const totalFillers = createMemo(() =>
    segments().reduce((acc, s) => acc + countFillersInText(s.text), 0)
  );
  const piiCount = createMemo(
    () => segments().filter((s) => detectSpokenPii(s.text) !== null).length
  );

  const expandedId = createMemo(() => {
    const sel = state.selectedSegmentId();
    if (sel) return sel;
    const ms = state.currentMs();
    return (
      segments().find((s) => ms >= s.start_ms && ms <= s.end_ms)?.id ?? null
    );
  });

  async function onTranscribe(): Promise<void> {
    const media = state.project().media;
    state.setBusyJob("Transcribe + Clean");
    try {
      if (isTauriRuntime()) {
        const t = await commands.transcribeLocal(media);
        state.pushUndo();
        state.setTranscript(t);
        state.setSelectedSegmentId(t.segments[0]?.id ?? null);
        state.notify("success", "Local Whisper + Glossary cleanup complete");
      } else {
        state.pushUndo();
        const t = sampleTranscript();
        state.setTranscript(t);
        state.setSelectedSegmentId(t.segments[0]?.id ?? null);
        state.notify(
          "success",
          "Preview: ran bilingual Whisper + Glossary cleanup"
        );
      }
    } catch (e) {
      state.notify("error", `Transcription failed: ${String(e)}`);
    } finally {
      state.setBusyJob(null);
    }
  }

  async function onFixSubtitles(): Promise<void> {
    const current = state.transcript();
    if (current.segments.length === 0) return;
    state.setBusyJob("Fix Subtitles (LLM)");
    try {
      if (isTauriRuntime()) {
        const fixed = await commands.fixSubtitlesLocal(current);
        state.pushUndo();
        state.setTranscript(fixed);
        state.notify("success", "Local Qwen + Glossary subtitle fix complete");
      } else {
        state.pushUndo();
        const gloss = state.glossary();
        state.setTranscript((tr) => ({
          ...tr,
          segments: tr.segments.map((seg) => {
            let nextText = stripFillersText(seg.text);
            for (const g of gloss) {
              const re = new RegExp(
                g.spoken_variant.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"),
                "gi"
              );
              nextText = nextText.replace(re, g.preferred_term);
            }
            return { ...seg, llm_draft: nextText, text: nextText };
          }),
        }));
        state.notify("success", "Preview: applied Glossary + Filler cleanup");
      }
    } catch (e) {
      state.notify("error", `Subtitle fix failed: ${String(e)}`);
    } finally {
      state.setBusyJob(null);
    }
  }

  function onCleanAllFillers(): void {
    const count = totalFillers();
    if (count === 0) return;
    state.pushUndo();
    state.setTranscript((tr) => ({
      ...tr,
      segments: tr.segments.map((seg) => ({
        ...seg,
        text: stripFillersText(seg.text),
      })),
    }));
    state.notify("success", `Cleaned ${count} filler word(s)`);
  }

  const visibleSegments = createMemo(() => {
    const q = filterQ();
    const onlyPii = state.subOnlyPii();
    return segments()
      .map((seg, idx) => ({ seg, idx }))
      .filter(({ seg }) => {
        if (q && !seg.text.toLowerCase().includes(q)) return false;
        if (onlyPii && detectSpokenPii(seg.text) === null) return false;
        return true;
      });
  });

  return (
    <>
      <div class="tc-insp-header">
        <div class="tc-insp-header__title">
          <span>Subtitles</span>
          <span class="tc-insp-header__count tc-mono">{segments().length}</span>
        </div>
        <div class="tc-flex-row tc-gap-1">
          <button
            class="tc-btn tc-btn--ghost tc-btn--sm"
            disabled={busy() || segments().length === 0}
            title="Run local Qwen2.5 + Glossary subtitle polish"
            onClick={() => void onFixSubtitles()}
          >
            <IconSparkle />
            Fix
          </button>
          <button
            class="tc-btn tc-btn--sm"
            disabled={busy()}
            onClick={() => void onTranscribe()}
          >
            {busy() ? "Running…" : "Transcribe"}
          </button>
        </div>
      </div>

      <div class="tc-insp-toolbar">
        <div class="tc-search-wrap">
          <span class="tc-search-wrap__icon">
            <IconSearch />
          </span>
          <input
            class="tc-search-input"
            placeholder="Filter subtitles…"
            value={state.subFilter()}
            onFocus={() => state.setEditingText(true)}
            onBlur={() => state.setEditingText(false)}
            onInput={(e) => state.setSubFilter(e.currentTarget.value)}
          />
        </div>

        <button
          class={
            state.showDiffs()
              ? "tc-filter-btn tc-filter-btn--active"
              : "tc-filter-btn"
          }
          title="Toggle inline 3-way ASR → LLM → User changes (D)"
          aria-pressed={state.showDiffs()}
          onClick={() => state.setShowDiffs(!state.showDiffs())}
        >
          Changes
        </button>

        <Show when={totalFillers() > 0}>
          <button
            class="tc-filter-btn"
            title="Strip speech fillers in one undoable step"
            onClick={onCleanAllFillers}
          >
            Fillers ({totalFillers()})
          </button>
        </Show>

        <Show when={piiCount() > 0}>
          <button
            class={
              state.subOnlyPii()
                ? "tc-filter-btn tc-filter-btn--active"
                : "tc-filter-btn tc-filter-btn--warn"
            }
            title="Filter to segments with spoken PII"
            aria-pressed={state.subOnlyPii()}
            onClick={() => state.setSubOnlyPii(!state.subOnlyPii())}
          >
            PII ({piiCount()})
          </button>
        </Show>
      </div>

      <div class="tc-insp-body tc-insp-body--list">
        <For each={visibleSegments()}>
          {(entry) => {
            const seg = () => entry.seg;
            const idx = () => entry.idx;
            const isExpanded = () => expandedId() === seg().id;
            const piiKind = () => detectSpokenPii(seg().text);
            const isBeeped = () => state.beepedSegments().has(seg().id);
            const hasLlmChange = () =>
              seg().raw_asr.trim() !== seg().text.trim();
            const canMerge = () => idx() + 1 < segments().length;

            const diffResult = createMemo(() => {
              if (!isExpanded()) return { tokens: [], candidates: [] };
              return computeThreeWayDiff(
                seg().raw_asr,
                seg().llm_draft,
                seg().text
              );
            });

            return (
              <div
                class={isExpanded() ? "tc-row tc-row--active" : "tc-row"}
                onClick={() => {
                  state.setSelectedSegmentId(seg().id);
                  state.seekMs(seg().start_ms);
                }}
              >
                <Show
                  when={isExpanded()}
                  fallback={
                    <div class="tc-row__summary">
                      <span class="tc-row__time tc-mono">
                        {formatSegTime(seg().start_ms)}
                      </span>
                      <span class="tc-row__text">{seg().text}</span>
                      <div class="tc-row__dots">
                        <Show
                          when={isBeeped()}
                          fallback={
                            <Show when={piiKind() !== null}>
                              <span
                                class="tc-dot-ind tc-dot-ind--pii"
                                title="Spoken PII detected"
                              />
                            </Show>
                          }
                        >
                          <span
                            class="tc-dot-ind tc-dot-ind--beep"
                            title="Spoken PII Auto-Beep armed"
                          />
                        </Show>
                        <Show when={hasLlmChange()}>
                          <span
                            class="tc-dot-ind tc-dot-ind--llm"
                            title="Modified from raw ASR"
                          />
                        </Show>
                      </div>
                    </div>
                  }
                >
                  <div class="tc-row__editor">
                    <div class="tc-row__head">
                      <div class="tc-flex-row">
                        <span class="tc-mono">
                          {formatSegTime(seg().start_ms)} –{" "}
                          {formatSegTime(seg().end_ms)}
                        </span>
                        <Show when={seg().speaker}>
                          {(sp) => <span class="tc-muted">· {sp()}</span>}
                        </Show>
                      </div>

                      <div class="tc-row__actions">
                        <Show when={piiKind()}>
                          {(kind) => (
                            <button
                              class={
                                isBeeped()
                                  ? "tc-btn tc-btn--sm tc-btn--active"
                                  : "tc-btn tc-btn--ghost tc-btn--sm"
                              }
                              title={`Toggle Spoken PII Auto-Beep (${kind()})`}
                              onClick={(e) => {
                                e.stopPropagation();
                                state.pushUndo();
                                const id = seg().id;
                                const next = new Set(state.beepedSegments());
                                const wasPresent = next.has(id);
                                if (wasPresent) {
                                  next.delete(id);
                                  state.notify(
                                    "info",
                                    "Removed audio beep marker"
                                  );
                                } else {
                                  next.add(id);
                                  state.notify(
                                    "success",
                                    `Armed Spoken PII Auto-Beep (${kind()})`
                                  );
                                }
                                state.setBeepedSegments(next);
                              }}
                            >
                              <IconVolumeOff />
                              {isBeeped() ? "Beeped" : "Beep"}
                            </button>
                          )}
                        </Show>

                        <button
                          class="tc-btn tc-btn--ghost tc-btn--sm tc-btn--icon"
                          title="Split segment"
                          onClick={(e) => {
                            e.stopPropagation();
                            const cur = seg();
                            const words = cur.text.trim().split(/\s+/).filter(Boolean);
                            if (words.length < 2) return;
                            state.pushUndo();
                            const mid = Math.floor(words.length / 2);
                            const leftTxt = words.slice(0, mid).join(" ");
                            const rightTxt = words.slice(mid).join(" ");
                            const midMs =
                              cur.start_ms +
                              Math.floor(Math.max(0, cur.end_ms - cur.start_ms) / 2);
                            const nextSeg: TranscriptSegment = {
                              id: newUuid(),
                              start_ms: midMs,
                              end_ms: cur.end_ms,
                              raw_asr: cur.raw_asr,
                              llm_draft: null,
                              text: rightTxt,
                              speaker: cur.speaker,
                              words: [],
                            };
                            const targetIdx = idx();
                            state.setTranscript((tr) => {
                              const nextList = [...tr.segments];
                              nextList[targetIdx] = {
                                ...cur,
                                end_ms: midMs,
                                text: leftTxt,
                              };
                              nextList.splice(targetIdx + 1, 0, nextSeg);
                              return { ...tr, segments: nextList };
                            });
                            state.notify("info", "Split subtitle segment");
                          }}
                        >
                          <IconScissors />
                        </button>

                        <Show when={canMerge()}>
                          <button
                            class="tc-btn tc-btn--ghost tc-btn--sm tc-btn--icon"
                            title="Merge with next segment"
                            onClick={(e) => {
                              e.stopPropagation();
                              const targetIdx = idx();
                              if (targetIdx + 1 >= segments().length) return;
                              state.pushUndo();
                              state.setTranscript((tr) => {
                                const nextList = [...tr.segments];
                                const cur = nextList[targetIdx];
                                const nxt = nextList[targetIdx + 1];
                                if (!cur || !nxt) return tr;
                                nextList.splice(targetIdx, 2, {
                                  ...cur,
                                  end_ms: nxt.end_ms,
                                  text: `${cur.text.trim()} ${nxt.text.trim()}`,
                                  raw_asr: `${cur.raw_asr.trim()} ${nxt.raw_asr.trim()}`,
                                });
                                return { ...tr, segments: nextList };
                              });
                              state.notify("info", "Merged with next segment");
                            }}
                          >
                            <IconMerge />
                          </button>
                        </Show>

                        <button
                          class="tc-btn tc-btn--ghost tc-btn--sm tc-btn--icon"
                          title="Delete segment"
                          onClick={(e) => {
                            e.stopPropagation();
                            const targetIdx = idx();
                            state.pushUndo();
                            state.setTranscript((tr) => ({
                              ...tr,
                              segments: tr.segments.filter((_, i) => i !== targetIdx),
                            }));
                            state.notify(
                              "warning",
                              "Deleted segment — Ctrl/⌘+Z to undo"
                            );
                          }}
                        >
                          <IconClose />
                        </button>
                      </div>
                    </div>

                    <ImeSafeTextarea
                      value={seg().text}
                      onFocus={() => {
                        state.setEditingText(true);
                        state.setSelectedSegmentId(seg().id);
                      }}
                      onCommit={(nextVal) => {
                        const targetId = seg().id;
                        state.setTranscript((tr) => ({
                          ...tr,
                          segments: tr.segments.map((s) =>
                            s.id === targetId ? { ...s, text: nextVal } : s
                          ),
                        }));
                        state.setDirty(true);
                      }}
                      onBlur={(finalVal) => {
                        state.setEditingText(false);
                        const cur = seg();
                        if (finalVal.trim() !== cur.raw_asr.trim() && isTauriRuntime()) {
                          const diff: EditDiff = {
                            raw_asr: cur.raw_asr,
                            llm_draft: cur.llm_draft,
                            user_final: finalVal,
                            edit_type: "word_fix",
                            audio_clip_ref: null,
                          };
                          void commands
                            .recordEditDiff(
                              state.project(),
                              diff,
                              state.optInTelemetry()
                            )
                            .catch(() => {});
                        }
                      }}
                    />

                    {/* Clean Inline 3-Way Diff */}
                    <Show
                      when={
                        state.showDiffs() && diffResult().tokens.length > 0
                      }
                    >
                      <div
                        class="tc-row__diff"
                        title="Red strikethrough: raw ASR removed · Green underline: LLM fix · Blue underline: your edit"
                      >
                        <For each={diffResult().tokens}>
                          {(tok) => {
                            let cls = "";
                            if (tok.origin === "deleted_asr") cls = "tc-diff-del";
                            else if (tok.origin === "inserted_llm")
                              cls = "tc-diff-llm";
                            else if (tok.origin === "inserted_user")
                              cls = "tc-diff-user";
                            if (tok.is_filler && tok.origin !== "deleted_asr") {
                              cls = cls ? `${cls} tc-diff-filler` : "tc-diff-filler";
                            }
                            return <span class={cls}>{tok.text}</span>;
                          }}
                        </For>
                      </div>
                    </Show>

                    {/* 1-Click Glossary Suggestion Pills */}
                    <Show when={diffResult().candidates.length > 0}>
                      <div class="tc-row__chips">
                        <For each={diffResult().candidates}>
                          {(cand) => {
                            const alreadySaved = () =>
                              state
                                .glossary()
                                .some(
                                  (g) =>
                                    g.spoken_variant.toLowerCase() ===
                                      cand.spoken_variant.toLowerCase() &&
                                    g.preferred_term.toLowerCase() ===
                                      cand.preferred_term.toLowerCase()
                                );

                            return (
                              <button
                                class={
                                  alreadySaved()
                                    ? "tc-gloss-link tc-gloss-link--saved"
                                    : "tc-gloss-link"
                                }
                                onClick={(e) => {
                                  e.stopPropagation();
                                  const s = cand.spoken_variant;
                                  const p = cand.preferred_term;
                                  if (isTauriRuntime()) {
                                    void commands
                                      .upsertGlossaryTerm(s, p)
                                      .then((entries) => {
                                        state.setGlossary(entries);
                                        state.notify(
                                          "success",
                                          `Saved to Glossary: ${s} → ${p}`
                                        );
                                      })
                                      .catch((err) =>
                                        state.notify(
                                          "error",
                                          `Glossary error: ${String(err)}`
                                        )
                                      );
                                  } else {
                                    state.setGlossary((prev) => {
                                      const existing = prev.find(
                                        (g) =>
                                          g.spoken_variant.toLowerCase() ===
                                          s.toLowerCase()
                                      );
                                      if (existing) {
                                        return prev.map((g) =>
                                          g.spoken_variant.toLowerCase() ===
                                          s.toLowerCase()
                                            ? {
                                                ...g,
                                                preferred_term: p,
                                                hit_count: g.hit_count + 1,
                                              }
                                            : g
                                        );
                                      }
                                      return [
                                        ...prev,
                                        {
                                          spoken_variant: s,
                                          preferred_term: p,
                                          hit_count: 1,
                                        },
                                      ];
                                    });
                                    state.notify(
                                      "success",
                                      `Added to Glossary: ${s} → ${p}`
                                    );
                                  }
                                }}
                              >
                                {alreadySaved()
                                  ? `✓ In glossary: ${cand.preferred_term}`
                                  : `+ Add '${cand.preferred_term}' to glossary`}
                              </button>
                            );
                          }}
                        </For>
                      </div>
                    </Show>
                  </div>
                </Show>
              </div>
            );
          }}
        </For>
      </div>
    </>
  );
};
