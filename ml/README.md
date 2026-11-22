# TinyCut ML Pipelines (PyTorch -> ONNX / GGUF)

Planned training & evaluation scripts (see `../BASELINE.md` and `../WORKFLOW.md`):

- `text_fixer/` — Qwen2.5/Qwen3 LoRA bilingual VN/EN subtitle post-processor (70% general + 30% opt-in diffs) -> `GGUF Q4_K_M` (local) + `FP16` (Cloud Boost).
- `vision_shield/` — Synthetic Vietnamese (1-line car & 2-line motorbike) + global license plate generator + opt-in hard negatives -> Apache-2.0 `D-FINE` / `RT-DETR` -> `ONNX FP16/INT8`.
- `asr/` — Phase 2 Whisper LoRA with strict verbatim CER filter (<40% rewrite) -> `whisper.cpp` GGML/GGUF.
- `eval/` — Golden Test Set benchmark gate (Vision Recall >= 99%, LLM term accuracy, ASR WER).
