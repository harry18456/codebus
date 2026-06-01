## Context

raw mirror（`codebus-core/src/vault/raw_sync.rs`）把 source repo 鏡像到各 vault 的 `.codebus/raw/code/`，agent 只讀這個鏡像、不讀 live repo。超過 `MAX_FILE_BYTES`（5 MiB）的檔案會在 `sync_with_scanner_into` 的 walk 迴圈內被跳過：不寫 mirror entry、bump `SyncSummary.oversized_skipped_files`、對 caller 提供的 `warn_sink`（生產環境為 stderr）輸出一行 `mirror skip: oversized ...`。

問題：warn line 只進 operator 的 stderr，**讀鏡像的 agent 對被跳過的大檔毫無所悉**，蓋架構頁時可能漏掉有結構意義的大檔。本設計把這些跳過項以一份低噪音的彙整 manifest 暴露到 agent 讀得到的鏡像目錄內。

中控評估此項為低價值（>5 MiB 多為 minified/vendored/binary blob），故設計刻意克制：單一彙整檔、僅路徑＋大小、無內容。

關鍵既有約束（已 ground 自 raw_sync.rs）：
- `sync_with_scanner_into` 開頭 `if raw_code_dir.exists() { remove_dir_all }` 再 `create_dir_all`（raw_sync.rs 約 194-197）— 每次 sync 都全量重建目的目錄。
- oversized 判斷與 skip 在 walk 迴圈內（約 254-275），與 PII 分支互斥（`continue`）。
- `walk_source_for_signal` 是 drift-detection 專用、走 source repo、無 warn sink。

## Goals / Non-Goals

**Goals:**

- 讓被跳過的 >5 MiB 檔對讀 `.codebus/raw/code/` 的 agent 可見，作為結構訊號。
- 低噪音：單一彙整 manifest，不污染檔樹，不誘使 agent 逐檔 Read。
- 完全不破壞既有 operator surface（warn line＋`oversized_skipped_files` counter）。
- 跨平台輸出穩定（forward-slash 路徑、依路徑排序）。

**Non-Goals:**

- 不做 per-file stub。
- 不把大檔內容（含截斷）寫進鏡像。
- 不改 5 MiB 門檻或跳過行為本身。
- 不動 `walk_source_for_signal`。
- 不抽象化（無 trait、無可插拔 writer）。
- 不對 manifest 內容跑 PII scanner（manifest 無檔案內容）。

## Decisions

### 在 walk 期間收集 oversized 條目、迴圈結束後寫單一 manifest

在 `sync_with_scanner_into` 函式作用域新增一個 `Vec<(String, u64)>`（forward-slash 相對路徑＋位元組數）。現有 oversized 分支除了既有的 warn line＋counter bump 外，多 push 一筆 `(rel_str, meta.len())`。walk 迴圈結束、`Ok(summary)` 之前，若該 Vec 非空，排序後格式化並 `fs::write` 到 `raw_code_dir.join("_codebus-oversized.md")`。

- 為什麼在迴圈內收集而非另開一次 walk：避免重複 I/O，且 oversized 判斷已在迴圈內、`rel_str` 已算好。
- 為什麼是迴圈結束後一次寫出：單一彙整檔比逐檔 append 簡單、可排序、可一致性處理。

替代方案（否決）：per-file stub（污染檔樹、誘逐檔 Read）；把清單塞進既有 `manifest.yaml`（那是 vault metadata 不在 `raw/code/`、agent 不一定讀）。

### manifest 位置與命名：`raw_code_dir/_codebus-oversized.md`

寫在 `raw_code_dir` 根（生產環境 `.codebus/raw/code/_codebus-oversized.md`）。agent 用 Glob/Read 自然會遇到；`.md` 對 Obsidian-compatible vault 自然；前導底線讓它在檔列表排前且明顯為 codebus 產物。

- 不會被重新 mirror／不污染 source_signal：manifest 在 `<repo>/.codebus/raw/code/` 之下，而 walk 走 source repo root、第一段 `.codebus` 即落入 `ALWAYS_SKIP_AT_ROOT` 被排除。故 manifest 絕不會被當成 source 檔重新鏡像，也不會計入 `walk_source_for_signal`。

### idempotency 由既有 `remove_dir_all` 保證、不新增刪除邏輯

「重 sync 覆蓋舊 manifest」「從有變無刪掉 stale manifest」皆由 `sync_with_scanner_into` 開頭整個 `remove_dir_all(raw_code_dir)` 自動成立：每次 sync 都是全新空目錄，上一輪的 manifest 已連同整個目錄被清掉，只有本輪確有 oversized 時才會重新寫出。因此本設計**不**新增任何「偵測並刪除舊 manifest」的程式碼。

替代方案（否決）：在無 oversized 時主動 `remove_file` manifest — 多餘，因為目錄已被全量重建。

### manifest 內容格式：header＋每檔一行（路徑＋bytes），依路徑排序

格式（apply 時定稿、以下為定義性內容）：
- 第一行起為 header：說明這些檔內容已省略、因超過 5 MiB、僅列出供結構認知（「content omitted, exceeds 5 MiB limit, listed for structural awareness」之意）。
- 其後一行一檔，每行＝forward-slash 相對路徑＋位元組數（例如 `- dist/bundle.js — 8388608 bytes`）。
- entry 在寫出前依路徑（`String` 序）排序 → 跨平台（`ignore` crate walk order 不保證一致）輸出穩定、利於測試斷言。

### manifest 是額外 surface，既有 counter／warn line 不動

oversized 分支的 `oversized_skipped_files += 1` 與 `writeln!(warn_sink, "mirror skip: ...")`（含其 best-effort 吞錯語意）完全保留。新增的收集只是 `Vec::push`，不參與 warn sink、不影響 counter、不改回傳 summary 結構。

## Implementation Contract

**Observable behavior**

- 一次 `sync_with_scanner_into` 跑完後，若該次有 ≥1 個 oversized 跳過 → `raw_code_dir/_codebus-oversized.md` 存在，內含 header＋每個被跳過檔一行（forward-slash 路徑＋bytes），entry 依路徑排序，且不含任何被跳過檔的內容。
- 若該次 0 個 oversized 跳過 → `raw_code_dir/_codebus-oversized.md` 不存在（含「前一輪有、這輪無」的情境，因目錄被全量重建）。
- 既有 `SyncSummary.oversized_skipped_files`、`mirror skip: oversized ...` warn line、warn-sink 失敗吞錯不 abort 等行為**逐一不變**。
- `walk_source_for_signal` 不寫 manifest、行為不變。

**Data shape**

- 函式作用域內新增 `Vec<(String, u64)>`（rel forward-slash path, byte len）。`SyncSummary` 結構**不**改動。
- manifest 檔名常數：`_codebus-oversized.md`（建議以 `const` 定義於 raw_sync.rs）。

**Failure modes**

- manifest 寫入採 best-effort，與既有 oversized warn-line 哲學一致：`fs::write` 失敗時 SHALL 吞錯、不 abort 整個 sync（counter／skip 才是 load-bearing）。apply 時以 `let _ = fs::write(...)` 或等效方式處理並加註解說明。

**Acceptance criteria（驗證目標，對應 spec scenario 與 tasks 測試）**

- 單元測試（`raw_sync.rs` tests module，沿用 `run_sync`／`write` helper）：
  - oversized → manifest 存在、含路徑＋bytes、不含內容、small 檔仍鏡像（對應 spec「writes an agent-visible manifest」）。
  - 無 oversized → manifest 不存在（對應「No oversized files leaves no manifest」）。
  - 連續兩次 sync（先有 oversized 後無）→ 第二次後 manifest 不存在（對應「does not leave a stale manifest」）。
  - 多個 oversized → manifest 含全部、依路徑排序（對應 Example）。
  - 既有 oversized 測試（`files_over_5_mib_are_skipped` 等）仍綠（counter／warn line 不回歸）。
- `cargo test -p codebus-core` 全綠、`cargo clippy --workspace` 無新 warning。

**In scope**

- `codebus-core/src/vault/raw_sync.rs`：oversized 分支收集、迴圈後寫 manifest、格式化 helper、新單元測試、manifest 檔名常數。

**Out of scope**

- 任何 CLI／app／IPC 改動（manifest 是 agent 讀的鏡像產物，無需新 surface）。
- `walk_source_for_signal`、5 MiB 門檻、PII 流程。
- `docs/BACKLOG.md`（依使用者指示不動）。

## Risks / Trade-offs

- [source repo 根有同名 `_codebus-oversized.md`] → 機率極低；本設計於迴圈結束後寫出，會覆蓋同位置的鏡像副本。屬可接受 edge，不額外處理；apply 時於 helper 加一行註解說明此取捨。
- [agent 可能仍忽略 manifest] → 屬低價值項已知限制；本變更只負責提供結構訊號，不保證 agent 消費。
- [大量 oversized 檔使 manifest 變長] → 每檔僅一行、純文字，體積可忽略；不分頁。

## Migration Plan

無資料遷移。純執行期產物，下一次任何觸發 raw sync 的 verb（init／goal 等）自然產生；舊 vault 重 sync 即補上。無 rollback 顧慮（移除程式碼即回到不寫 manifest，既有鏡像不受影響）。

## Open Questions

無。manifest 確切行格式（分隔符與 header 文字）於 apply 定稿，受 spec scenario（含路徑＋bytes、排序、不含內容）約束。
