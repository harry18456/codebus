## 1. RED — `TrackedProvider tags usage records with default_module` 規約測試

對應 spec `llm-provider / TrackedProvider tags usage records with default_module`。

- [x] 1.1 [P] 於 `sidecar/tests/providers/test_default_module.py` 加 `test_tracked_provider_records_default_module_on_embed`（建構帶 `default_module="kb_build"`，跑一次 embed，斷言 `token_usage.jsonl` 有 `"module": "kb_build"` 行）
- [x] 1.2 [P] 於 `test_default_module.py` 加 `test_tracked_provider_records_default_module_on_chat`（同上但 chat path，TrackedProvider tags usage records with default_module 應同樣套用）
- [x] 1.3 [P] 於 `test_default_module.py` 加 `test_omitting_default_module_writes_empty_string`（M1 backward compat：建構不帶 `default_module` → record 的 module 欄為 `""`）
- [x] 1.4 [P] 於 `test_default_module.py` 加 `test_failure_path_still_records_default_module`（內 provider 拋例外時，TrackedProvider tags usage records with default_module 仍套到失敗 record，retry cost 歸正確 subsystem）

## 2. GREEN — 實作 `TrackedProvider tags usage records with default_module`

- [x] 2.1 於 `providers/tracked.py::TrackedProvider.__init__` 加 `default_module: str | None = None` keyword-only 參數，存於 `self._default_module`（normalize 到空字串）
- [x] 2.2 於 `providers/tracked.py::TrackedProvider.chat` 內 `self._tracker.record(...)` 呼叫加 `module=self._default_module`
- [x] 2.3 於 `providers/tracked.py::TrackedProvider.embed` 內 `self._tracker.record(...)` 呼叫加 `module=self._default_module`
- [x] 2.4 確認 `providers/tracked.py` 失敗路徑（`except BaseException`）也走相同 `default_module` 注入邏輯（M1 失敗路徑只寫 `llm_calls.jsonl`，不寫 `token_usage.jsonl`，故無 record 需要套 module；測試 1.4 已驗證契約）
- [x] 2.5 執行 `uv run pytest sidecar/tests/providers/test_default_module.py` 確認 1.1 ~ 1.4 全綠

## 3. RED — `UsageTracker writes token_usage.jsonl` 「exactly one line」契約測試

對應 spec `usage-tracking / UsageTracker writes token_usage.jsonl` Scenario「Module field reflects TrackedProvider's default_module」（含「no second line with same tuple」斷言）。

- [x] 3.1 [P] 於 `sidecar/tests/api/test_kb_build_production.py` 修 `test_usage_tracker_writes_to_workspace_scoped_path` 加新斷言：每個 batch 對應**恰好一行**（不是兩行）—— `len(embed_lines) == batches_embedded`，反映 `UsageTracker writes token_usage.jsonl` 的 dedup 不變式；fixture 加 TrackedProvider 包 SpyProvider 模擬 production wiring
- [x] 3.2 [P] 於 `sidecar/tests/kb/test_knowledge_base.py` 替換既有 `test_usage_tracker_records_one_entry_per_batch` 為 `test_kb_build_does_not_call_tracker_directly`：用 raw SpyProvider（無 TrackedProvider 包裝）+ 觀察 tracker 檔內容，斷言 KnowledgeBase 自身不再寫 tracker

## 4. GREEN — 移除 `KnowledgeBase` 的手動 record

- [x] 4.1 於 `kb/knowledge_base.py::KnowledgeBase._run_batch`（或對應位置）移除 `self._tracker.record(usage=response.usage, module="kb_build")` 那行
- [x] 4.2 於 `kb/knowledge_base.py::KnowledgeBase.__init__` 把 `usage_tracker` 參數標記 deprecated（comment「retained for backward compat; TrackedProvider now records via default_module」），但不移除—— Non-Goal 明文保留
- [x] 4.3 執行 `uv run pytest sidecar/tests/api/test_kb_build_production.py sidecar/tests/kb/` 確認 3.1 / 3.2 全綠 + 既有 KB 測無 regression

## 5. GREEN — `wire_kb_dependencies` 帶 `default_module="kb_build"`

- [x] 5.1 於 `api/__init__.py::_make_provider_factory` 內建構 `TrackedProvider(...)` 時加 `default_module="kb_build"`，把 KB build 路徑與 spec `TrackedProvider tags usage records with default_module` 的 happy-path scenario 接上
- [x] 5.2 執行 `uv run pytest sidecar/tests/test_wire_kb_dependencies.py` 確認既有 5 條測無 regression

## 6. 文件更新

- [x] 6.1 於 `docs/llm-provider.md §三-bis`（OpenAI embedding 段）補一小段：`default_module` 為 wrapping pattern 的 single-source-of-truth；KB / qa_agent / generator 等 module 帶自己的字串，避免重複記帳
- [x] 6.2 於 `docs/module-2-kb-builder.md §七`（production wiring 段）補一行：`UsageTracker` 由 `TrackedProvider` 的 `default_module="kb_build"` 自動帶入,KB build 不再手動 `tracker.record(...)`
- [x] 6.3 於 `CLAUDE.md`「最近一筆 in-progress」改指 `usage-tracker-dedup`，並於 sidecar 描述補「TrackedProvider 加 `default_module` 為 token_usage.jsonl 唯一記錄路徑」

## 7. 驗證與 commit gate

- [x] 7.1 執行 `uv run pytest sidecar/tests/providers/` 確認 provider 層全綠（81 passed）
- [x] 7.2 執行 `uv run pytest sidecar/tests/api/ sidecar/tests/kb/` 確認上層無 regression（168 passed, 6 skipped）
- [x] 7.3 執行 `uv run pytest` 完整 suite 無 regression（499 passed, 17 skipped）
- [x] 7.4 執行 `pre-commit run --all-files` 全綠
- [x] 7.5 手動煙霧測重跑（同 `kb-build-production-wiring` 9.5 的 happy path）：`POST /kb/build` → 等 done → 檢 `<workspace>/token_usage.jsonl` **行數 = batches_embedded（不是 ×2）**，且每行 `module="kb_build"` ✅（3 batches → 3 lines, all module=kb_build）
