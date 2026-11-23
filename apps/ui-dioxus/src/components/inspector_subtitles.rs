use crate::bridge::{
    cmd_record_edit_diff, cmd_transcribe_local, cmd_upsert_glossary_term, is_tauri_runtime,
};
use crate::components::icons::{
    IconClose, IconMerge, IconScissors, IconSearch, IconVolumeOff,
};
use crate::domain_ext::{
    DiffOrigin, compute_three_way_diff, count_fillers_in_text, detect_spoken_pii, format_seg_time,
    strip_fillers_text,
};
use crate::state::{StudioState, ToastSeverity, sample_transcript};
use dioxus::prelude::*;
use tinycut_core_domain::{EditDiff, EditType, GlossaryEntry, TranscriptSegment};
use uuid::Uuid;

#[component]
pub fn SubtitlesInspector() -> Element {
    let mut state = use_context::<StudioState>();

    let segments = state.transcript.read().segments.clone();
    let show_diffs = *state.show_diffs.read();
    let selected_id = *state.selected_segment_id.read();
    let current_ms = *state.current_ms.read();
    let glossary = state.glossary.read().clone();
    let beeped = state.beeped_segments.read().clone();
    let busy = state.busy_job.read().is_some();
    let filter_q = state.sub_filter.read().trim().to_lowercase();
    let only_pii = *state.sub_only_pii.read();

    let total_fillers: usize = segments
        .iter()
        .map(|s| count_fillers_in_text(&s.text))
        .sum();
    let pii_count = segments
        .iter()
        .filter(|s| detect_spoken_pii(&s.text).is_some())
        .count();

    let on_transcribe = move |_| {
        let media = state.project.read().media.clone();
        state.busy_job.set(Some("Transcribe + Clean".into()));
        spawn(async move {
            if is_tauri_runtime() {
                match cmd_transcribe_local(media).await {
                    Ok(t) => {
                        state.push_undo();
                        let first = t.segments.first().map(|s| s.id);
                        state.transcript.set(t);
                        state.selected_segment_id.set(first);
                        state.notify(
                            ToastSeverity::Success,
                            "Local Whisper + Glossary cleanup complete",
                        );
                    }
                    Err(e) => {
                        state.notify(ToastSeverity::Error, format!("Transcription failed: {e}"));
                    }
                }
            } else {
                state.push_undo();
                let t = sample_transcript();
                state.transcript.set(t);
                state.notify(
                    ToastSeverity::Success,
                    "Preview: ran bilingual Whisper + Glossary cleanup",
                );
            }
            state.busy_job.set(None);
        });
    };

    let on_clean_all_fillers = move |_| {
        if total_fillers == 0 {
            return;
        }
        state.push_undo();
        for seg in &mut state.transcript.write().segments {
            seg.text = strip_fillers_text(&seg.text);
        }
        state.notify(
            ToastSeverity::Success,
            format!("Cleaned {total_fillers} filler word(s)"),
        );
    };

    // Resolve which single row is expanded (selected row, or currently playing row if none selected)
    let expanded_id = selected_id.or_else(|| {
        segments
            .iter()
            .find(|s| current_ms >= s.start_ms && current_ms <= s.end_ms)
            .map(|s| s.id)
    });

    rsx! {
        div { class: "tc-insp-header",
            div { class: "tc-insp-header__title",
                span { "Subtitles" }
                span { class: "tc-insp-header__count tc-mono", "{segments.len()}" }
            }
            button {
                class: "tc-btn tc-btn--sm",
                disabled: busy,
                onclick: on_transcribe,
                if busy { "Running…" } else { "Transcribe" }
            }
        }

        div { class: "tc-insp-toolbar",
            div { class: "tc-search-wrap",
                span { class: "tc-search-wrap__icon", IconSearch {} }
                input {
                    class: "tc-search-input",
                    placeholder: "Filter subtitles…",
                    value: "{state.sub_filter}",
                    onfocus: move |_| state.editing_text.set(true),
                    onblur: move |_| state.editing_text.set(false),
                    oninput: move |e| state.sub_filter.set(e.value()),
                }
            }

            button {
                class: if show_diffs { "tc-filter-btn tc-filter-btn--active" } else { "tc-filter-btn" },
                title: "Toggle inline 3-way ASR → LLM → User changes (D)",
                "aria-pressed": "{show_diffs}",
                onclick: move |_| state.show_diffs.set(!show_diffs),
                "Changes"
            }

            if total_fillers > 0 {
                button {
                    class: "tc-filter-btn",
                    title: "Strip speech fillers in one undoable step",
                    onclick: on_clean_all_fillers,
                    "Fillers ({total_fillers})"
                }
            }

            if pii_count > 0 {
                button {
                    class: if only_pii {
                        "tc-filter-btn tc-filter-btn--active"
                    } else {
                        "tc-filter-btn tc-filter-btn--warn"
                    },
                    title: "Filter to segments with spoken PII",
                    "aria-pressed": "{only_pii}",
                    onclick: move |_| state.sub_only_pii.set(!only_pii),
                    "PII ({pii_count})"
                }
            }
        }

        div { class: "tc-insp-body tc-insp-body--list",
            for (idx, seg) in segments.iter().cloned().enumerate() {
                if (!filter_q.is_empty() && !seg.text.to_lowercase().contains(&filter_q))
                    || (only_pii && detect_spoken_pii(&seg.text).is_none())
                {
                    // Filtered out
                } else {
                    {
                        let is_expanded = expanded_id == Some(seg.id);
                        let pii_kind = detect_spoken_pii(&seg.text);
                        let is_beeped = beeped.contains(&seg.id);
                        let has_llm_change = seg.raw_asr.trim() != seg.text.trim();
                        let seg_id = seg.id;
                        let can_merge = idx + 1 < segments.len();
                        let raw_asr_clone = seg.raw_asr.clone();
                        let llm_draft_clone = seg.llm_draft.clone();

                        // Compute 3-way word diff ONLY for the single expanded row
                        let (diff_tokens, candidates) = if is_expanded {
                            compute_three_way_diff(&seg.raw_asr, seg.llm_draft.as_deref(), &seg.text)
                        } else {
                            (Vec::new(), Vec::new())
                        };

                        rsx! {
                            div {
                                key: "{seg.id}",
                                class: if is_expanded { "tc-row tc-row--active" } else { "tc-row" },
                                onclick: move |_| {
                                    state.selected_segment_id.set(Some(seg_id));
                                    state.seek_ms(seg.start_ms);
                                },

                                if !is_expanded {
                                    // Calm Collapsed Row: Timecode + 2-Line Clamped Text + Trailing Status Dots
                                    div { class: "tc-row__summary",
                                        span { class: "tc-row__time tc-mono", "{format_seg_time(seg.start_ms)}" }
                                        span { class: "tc-row__text", "{seg.text}" }
                                        div { class: "tc-row__dots",
                                            if is_beeped {
                                                span { class: "tc-dot-ind tc-dot-ind--beep", title: "Spoken PII Auto-Beep armed" }
                                            } else if pii_kind.is_some() {
                                                span { class: "tc-dot-ind tc-dot-ind--pii", title: "Spoken PII detected" }
                                            }
                                            if has_llm_change {
                                                span { class: "tc-dot-ind tc-dot-ind--llm", title: "Modified from raw ASR" }
                                            }
                                        }
                                    }
                                } else {
                                    // In-Place Expanded Active Row
                                    div { class: "tc-row__editor",
                                        div { class: "tc-row__head",
                                            div { class: "tc-flex-row",
                                                span { class: "tc-mono", "{format_seg_time(seg.start_ms)} – {format_seg_time(seg.end_ms)}" }
                                                if let Some(sp) = seg.speaker.as_deref() {
                                                    span { class: "tc-muted", "· {sp}" }
                                                }
                                            }

                                            div { class: "tc-row__actions",
                                                if let Some(kind) = pii_kind {
                                                    button {
                                                        class: if is_beeped { "tc-btn tc-btn--sm tc-btn--active" } else { "tc-btn tc-btn--ghost tc-btn--sm" },
                                                        title: "Toggle Spoken PII Auto-Beep ({kind})",
                                                        onclick: move |evt| {
                                                            evt.stop_propagation();
                                                            state.push_undo();
                                                            let was_present = {
                                                                let mut set = state.beeped_segments.write();
                                                                if set.contains(&seg_id) {
                                                                    set.remove(&seg_id);
                                                                    true
                                                                } else {
                                                                    set.insert(seg_id);
                                                                    false
                                                                }
                                                            };
                                                            if was_present {
                                                                state.notify(ToastSeverity::Info, "Removed audio beep marker");
                                                            } else {
                                                                state.notify(ToastSeverity::Success, format!("Armed Spoken PII Auto-Beep ({kind})"));
                                                            }
                                                        },
                                                        IconVolumeOff {}
                                                        if is_beeped { "Beeped" } else { "Beep" }
                                                    }
                                                }

                                                button {
                                                    class: "tc-btn tc-btn--ghost tc-btn--sm tc-btn--icon",
                                                    title: "Split segment",
                                                    onclick: move |evt| {
                                                        evt.stop_propagation();
                                                        state.push_undo();
                                                        let did_split = {
                                                            let mut tr = state.transcript.write();
                                                            if let Some(curr) = tr.segments.get(idx).cloned() {
                                                                let words: Vec<&str> = curr.text.split_whitespace().collect();
                                                                if words.len() >= 2 {
                                                                    let mid = words.len() / 2;
                                                                    let left_txt = words[..mid].join(" ");
                                                                    let right_txt = words[mid..].join(" ");
                                                                    let mid_ms = curr.start_ms + (curr.end_ms.saturating_sub(curr.start_ms)) / 2;
                                                                    tr.segments[idx].end_ms = mid_ms;
                                                                    tr.segments[idx].text = left_txt;
                                                                    let next_seg = TranscriptSegment {
                                                                        id: Uuid::new_v4(),
                                                                        start_ms: mid_ms,
                                                                        end_ms: curr.end_ms,
                                                                        raw_asr: curr.raw_asr,
                                                                        llm_draft: None,
                                                                        text: right_txt,
                                                                        speaker: curr.speaker,
                                                                        words: vec![],
                                                                    };
                                                                    tr.segments.insert(idx + 1, next_seg);
                                                                    true
                                                                } else {
                                                                    false
                                                                }
                                                            } else {
                                                                false
                                                            }
                                                        };
                                                        if did_split {
                                                            state.notify(ToastSeverity::Info, "Split subtitle segment");
                                                        }
                                                    },
                                                    IconScissors {}
                                                }

                                                if can_merge {
                                                    button {
                                                        class: "tc-btn tc-btn--ghost tc-btn--sm tc-btn--icon",
                                                        title: "Merge with next segment",
                                                        onclick: move |evt| {
                                                            evt.stop_propagation();
                                                            state.push_undo();
                                                            let did_merge = {
                                                                let mut tr = state.transcript.write();
                                                                if idx + 1 < tr.segments.len() {
                                                                    let next = tr.segments.remove(idx + 1);
                                                                    let cur = &mut tr.segments[idx];
                                                                    cur.end_ms = next.end_ms;
                                                                    cur.text = format!("{} {}", cur.text.trim(), next.text.trim());
                                                                    cur.raw_asr = format!("{} {}", cur.raw_asr.trim(), next.raw_asr.trim());
                                                                    true
                                                                } else {
                                                                    false
                                                                }
                                                            };
                                                            if did_merge {
                                                                state.notify(ToastSeverity::Info, "Merged with next segment");
                                                            }
                                                        },
                                                        IconMerge {}
                                                    }
                                                }

                                                button {
                                                    class: "tc-btn tc-btn--ghost tc-btn--sm tc-btn--icon",
                                                    title: "Delete segment",
                                                    onclick: move |evt| {
                                                        evt.stop_propagation();
                                                        state.push_undo();
                                                        let did_delete = {
                                                            let mut tr = state.transcript.write();
                                                            if idx < tr.segments.len() {
                                                                tr.segments.remove(idx);
                                                                true
                                                            } else {
                                                                false
                                                            }
                                                        };
                                                        if did_delete {
                                                            state.notify(ToastSeverity::Warning, "Deleted segment — Ctrl/⌘+Z to undo");
                                                        }
                                                    },
                                                    IconClose {}
                                                }
                                            }
                                        }

                                        textarea {
                                            class: "tc-textarea",
                                            rows: "2",
                                            value: "{seg.text}",
                                            onclick: move |evt| evt.stop_propagation(),
                                            onfocus: move |_| {
                                                state.editing_text.set(true);
                                                state.selected_segment_id.set(Some(seg_id));
                                            },
                                            onblur: move |_| {
                                                state.editing_text.set(false);
                                                let current_text = state
                                                    .transcript
                                                    .read()
                                                    .segments
                                                    .get(idx)
                                                    .map(|s| s.text.clone())
                                                    .unwrap_or_default();
                                                if current_text.trim() != raw_asr_clone.trim() {
                                                    let project = state.project.read().clone();
                                                    let opt_in = *state.opt_in_telemetry.read();
                                                    let diff = EditDiff {
                                                        raw_asr: raw_asr_clone.clone(),
                                                        llm_draft: llm_draft_clone.clone(),
                                                        user_final: current_text,
                                                        edit_type: EditType::WordFix,
                                                        audio_clip_ref: None,
                                                    };
                                                    spawn(async move {
                                                        if is_tauri_runtime() {
                                                            let _ = cmd_record_edit_diff(project, diff, opt_in).await;
                                                        }
                                                    });
                                                }
                                            },
                                            oninput: move |evt| {
                                                if let Some(target) = state.transcript.write().segments.get_mut(idx) {
                                                    target.text = evt.value();
                                                }
                                                state.dirty.set(true);
                                            }
                                        }

                                        // Clean inline 3-way diff (no "DIFF" box wrapper)
                                        if show_diffs && !diff_tokens.is_empty() {
                                            div { class: "tc-row__diff",
                                                title: "Red strikethrough: raw ASR removed · Green underline: LLM fix · Blue underline: your edit",
                                                for (t_i, tok) in diff_tokens.iter().enumerate() {
                                                    {
                                                        let mut cls = match tok.origin {
                                                            DiffOrigin::Unchanged => String::new(),
                                                            DiffOrigin::DeletedAsr => String::from("tc-diff-del"),
                                                            DiffOrigin::InsertedLlm => String::from("tc-diff-llm"),
                                                            DiffOrigin::InsertedUser => String::from("tc-diff-user"),
                                                        };
                                                        if tok.is_filler && tok.origin != DiffOrigin::DeletedAsr {
                                                            cls.push_str(" tc-diff-filler");
                                                        }
                                                        rsx! {
                                                            span { key: "tok-{idx}-{t_i}", class: "{cls}", "{tok.text}" }
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // Plain text-link glossary suggestions
                                        if !candidates.is_empty() {
                                            div { class: "tc-row__chips",
                                                for cand in candidates {
                                                    {
                                                        let existing = glossary.iter().find(|g| {
                                                            g.spoken_variant.eq_ignore_ascii_case(&cand.spoken_variant)
                                                                && g.preferred_term.eq_ignore_ascii_case(&cand.preferred_term)
                                                        });
                                                        let already_saved = existing.is_some();
                                                        let spoken = cand.spoken_variant.clone();
                                                        let preferred = cand.preferred_term.clone();
                                                        rsx! {
                                                            button {
                                                                key: "chip-{spoken}-{preferred}",
                                                                class: if already_saved { "tc-gloss-link tc-gloss-link--saved" } else { "tc-gloss-link" },
                                                                onclick: move |evt| {
                                                                    evt.stop_propagation();
                                                                    let s = spoken.clone();
                                                                    let p = preferred.clone();
                                                                    spawn(async move {
                                                                        if is_tauri_runtime() {
                                                                            match cmd_upsert_glossary_term(s.clone(), p.clone()).await {
                                                                                Ok(entries) => {
                                                                                    state.glossary.set(entries);
                                                                                    state.notify(ToastSeverity::Success, format!("Saved to Glossary: {s} → {p}"));
                                                                                }
                                                                                Err(e) => state.notify(ToastSeverity::Error, format!("Glossary error: {e}")),
                                                                            }
                                                                        } else {
                                                                            {
                                                                                let mut list = state.glossary.write();
                                                                                if let Some(item) = list.iter_mut().find(|g| g.spoken_variant.eq_ignore_ascii_case(&s)) {
                                                                                    item.preferred_term = p.clone();
                                                                                    item.hit_count += 1;
                                                                                } else {
                                                                                    list.push(GlossaryEntry {
                                                                                        spoken_variant: s.clone(),
                                                                                        preferred_term: p.clone(),
                                                                                        hit_count: 1,
                                                                                    });
                                                                                }
                                                                            }
                                                                            state.notify(ToastSeverity::Success, format!("Added to Glossary: {s} → {p}"));
                                                                        }
                                                                    });
                                                                },
                                                                if already_saved {
                                                                    "✓ In glossary: {cand.preferred_term}"
                                                                } else {
                                                                    "+ Add '{cand.preferred_term}' to glossary"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
