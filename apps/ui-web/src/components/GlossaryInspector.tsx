import { For, createMemo, createSignal, type Component } from "solid-js";
import { commands, isTauriRuntime } from "../bridge";
import { useStudio } from "../state";
import { IconClose, IconPlus, IconSearch } from "./Icons";

export const GlossaryInspector: Component = () => {
  const state = useStudio();
  const [search, setSearch] = createSignal("");
  const [spokenIn, setSpokenIn] = createSignal("");
  const [preferredIn, setPreferredIn] = createSignal("");

  const filtered = createMemo(() => {
    const q = search().trim().toLowerCase();
    return state.glossary().filter(
      (e) =>
        !q ||
        e.spoken_variant.toLowerCase().includes(q) ||
        e.preferred_term.toLowerCase().includes(q)
    );
  });

  const promptPreview = createMemo(() => {
    const terms = state.glossary().map((e) => e.preferred_term);
    if (terms.length === 0) return "No glossary terms yet.";
    return `Glossary terms: ${terms.join(", ")}.`;
  });

  async function onAdd(): Promise<void> {
    const s = spokenIn().trim();
    const p = preferredIn().trim();
    if (!s || !p) {
      state.notify("warning", "Enter both a spoken variant and preferred term");
      return;
    }
    setSpokenIn("");
    setPreferredIn("");

    if (isTauriRuntime()) {
      try {
        const list = await commands.upsertGlossaryTerm(s, p);
        state.setGlossary(list);
        state.notify("success", `Saved to Glossary: ${s} → ${p}`);
      } catch (e) {
        state.notify("error", `Glossary save failed: ${String(e)}`);
      }
    } else {
      state.setGlossary((prev) => {
        const idx = prev.findIndex(
          (g) => g.spoken_variant.toLowerCase() === s.toLowerCase()
        );
        if (idx >= 0) {
          return prev.map((g, i) =>
            i === idx
              ? { ...g, preferred_term: p, hit_count: g.hit_count + 1 }
              : g
          );
        }
        return [
          ...prev,
          { spoken_variant: s, preferred_term: p, hit_count: 1 },
        ];
      });
      state.notify("success", `Added ${s} → ${p}`);
    }
  }

  return (
    <>
      <div class="tc-insp-header">
        <div class="tc-insp-header__title">
          <span>Personal Glossary</span>
          <span class="tc-insp-header__count tc-mono">
            {state.glossary().length}
          </span>
        </div>
        <span class="tc-form-row__sub">SQLite Local</span>
      </div>

      <div class="tc-insp-toolbar">
        <div class="tc-search-wrap">
          <span class="tc-search-wrap__icon">
            <IconSearch />
          </span>
          <input
            class="tc-search-input"
            placeholder="Filter glossary terms…"
            value={search()}
            onFocus={() => state.setEditingText(true)}
            onBlur={() => state.setEditingText(false)}
            onInput={(e) => setSearch(e.currentTarget.value)}
          />
        </div>
      </div>

      <div class="tc-insp-body">
        <div class="tc-insp-body--form">
          <div class="tc-group">
            <div class="tc-group__title">Teach New Term</div>
            <div class="tc-group__card">
              <div class="tc-form-row tc-form-row--stack">
                <div class="tc-grid-2">
                  <input
                    class="tc-input"
                    placeholder="Spoken (e.g. cơ sơ)"
                    value={spokenIn()}
                    onFocus={() => state.setEditingText(true)}
                    onBlur={() => state.setEditingText(false)}
                    onInput={(e) => setSpokenIn(e.currentTarget.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && !e.isComposing) void onAdd();
                    }}
                  />
                  <input
                    class="tc-input"
                    placeholder="Preferred (e.g. Cursor)"
                    value={preferredIn()}
                    onFocus={() => state.setEditingText(true)}
                    onBlur={() => state.setEditingText(false)}
                    onInput={(e) => setPreferredIn(e.currentTarget.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && !e.isComposing) void onAdd();
                    }}
                  />
                </div>
                <button class="tc-btn" onClick={() => void onAdd()}>
                  <IconPlus />
                  Add to Glossary
                </button>
              </div>
            </div>
          </div>

          <div class="tc-group">
            <div class="tc-group__title">Whisper initial_prompt</div>
            <div class="tc-prompt-box tc-mono">{promptPreview()}</div>
          </div>

          <div class="tc-group__title">
            <span>Saved Terms</span>
            <span class="tc-group__sub tc-mono">{filtered().length}</span>
          </div>
        </div>

        <div class="tc-insp-body--list">
          <For each={filtered()}>
            {(entry) => (
              <div class="tc-row">
                <div class="tc-flex-between">
                  <div class="tc-flex-row">
                    <span class="tc-muted">{entry.spoken_variant}</span>
                    <span class="tc-muted">→</span>
                    <span>{entry.preferred_term}</span>
                  </div>
                  <div class="tc-flex-row tc-gap-1">
                    <span class="tc-form-row__sub tc-mono">
                      ×{entry.hit_count}
                    </span>
                    <button
                      class="tc-btn tc-btn--ghost tc-btn--sm tc-btn--icon"
                      title="Remove term"
                      onClick={() => {
                        const s = entry.spoken_variant;
                        if (isTauriRuntime()) {
                          void commands
                            .deleteGlossaryTerm(s)
                            .then((list) => {
                              state.setGlossary(list);
                              state.notify("info", `Removed '${s}'`);
                            })
                            .catch((e) =>
                              state.notify("error", `Delete failed: ${String(e)}`)
                            );
                        } else {
                          state.setGlossary((prev) =>
                            prev.filter(
                              (g) =>
                                g.spoken_variant.toLowerCase() !== s.toLowerCase()
                            )
                          );
                          state.notify("info", `Removed '${s}'`);
                        }
                      }}
                    >
                      <IconClose />
                    </button>
                  </div>
                </div>
              </div>
            )}
          </For>
        </div>
      </div>
    </>
  );
};
