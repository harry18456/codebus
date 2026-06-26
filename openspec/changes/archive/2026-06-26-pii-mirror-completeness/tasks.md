## 1. 擴充 builtin 機密 pattern — Built-in Regex Pattern Coverage（Decision 1、Decision 2）

- [x] 1.1 [P] 在 `codebus-core/src/pii/scanners/regex_basic.rs` 既有測試模組先寫失敗測試（RED）：為「Decision 1: 新增九條高精度 builtin 機密 pattern（共 13 條）」各補 1 個 positive（真機密命中正確 `pattern_name` 與 severity）+ 1 個 negative（近似的正常字串/程式碼不誤遮），並補一條「Decision 2: OpenAI key 以 alternation 規避與 Anthropic key 的重疊（無 lookaround）」的 negative 測試（`sk-ant-...` 只命中 `anthropic-api-key`、不產生 `openai-api-key`）。驗證：`cargo test -p codebus-core regex_basic` 此時應 FAIL（pattern 尚未實作）。
- [x] 1.2 實作「Decision 1: 新增九條高精度 builtin 機密 pattern（共 13 條）」：在 `BUILTIN_PATTERNS` 追加 `github-pat`、`github-fine-grained-pat`、`slack-token`、`google-api-key`、`openai-api-key`、`stripe-secret-key`、`pem-private-key`、`jwt`、`db-connection-string`，severity 依 spec（JWT 為 Warn、其餘為 Critical）；依「Decision 2: OpenAI key 以 alternation 規避與 Anthropic key 的重疊（無 lookaround）」採 `sk-(?:proj-[A-Za-z0-9_\-]{20,}|[A-Za-z0-9]{20,})`，落實 Built-in Regex Pattern Coverage requirement。觀察行為：`RegexBasicScanner` 預設對各機密形狀產生對應 `pattern_name`/severity 的 match。驗證：1.1 的測試全數轉 PASS。
- [x] 1.3 確認 Scanner Selection from Config requirement 仍以「full built-in pattern set 加上 `patterns_extra`」構造 `RegexBasicScanner`，且最終 builtin pattern 數為 13。觀察行為：`builtin_pattern_count()`（見 3.1）回傳 13、`regex_basic` scanner 涵蓋全部 13 條 builtin。驗證：新增/更新斷言 `BUILTIN_PATTERNS.len() == 13` 的測試並通過 `cargo test -p codebus-core`。

## 2. 非 UTF-8 decode-scan 與不可掃描檔計數（Decision 3、Decision 4）

- [x] 2.1 [P] 在 `codebus-core` 既有 vault 測試（raw_sync 對應測試模組/`tests/`）先寫失敗測試（RED）：(a) 帶 UTF-16 LE BOM、內含 AWS key 的文字檔經 sync 後 destination 含 `[REDACTED:aws-access-key]` 且原 key 字元不出現；(b) 真二進位檔（無 BOM、非 UTF-8）仍 byte-identical copy 且 `SyncSummary.unscanned_files` +1；(c) 對應更新既有 `mask_mode_falls_through_to_copy_for_non_utf8`，使其只涵蓋「無 BOM 真二進位 fall through」語意。驗證：`cargo test -p codebus-core` 此時相關測試 FAIL。
- [x] 2.2 在 `SyncSummary`（`codebus-core/src/vault/raw_sync.rs` 與相關 `manifest.rs` 型別）新增 `unscanned_files: usize` 欄位，預設 0，落實「Decision 4: 不可掃描檔的可觀察性用彙總 counter（不做 per-file manifest）」的資料面。觀察行為：sync 回傳的 summary 帶有可讀取的 `unscanned_files` 計數。驗證：型別編譯通過且 2.1(b) 測試可引用該欄位。
- [x] 2.3 在 `raw_sync` 的讀檔分支實作「Decision 3: 非 UTF-8 改為 UTF-16 BOM decode-scan，無法解碼者維持 verbatim copy」：UTF-8 失敗時偵測 `FF FE`/`FE FF`/`EF BB BF` BOM → 轉碼 UTF-8 後掃描（clean 維持原始 bytes byte-identical、命中則寫 UTF-8 遮罩結果）；無可辨識 BOM 的真二進位 fall through verbatim copy 並依「Decision 4: 不可掃描檔的可觀察性用彙總 counter（不做 per-file manifest）」累加 `unscanned_files`。觀察行為：BOM-marked UTF-16 機密被遮罩、二進位被計數。驗證：2.1 全部測試轉 PASS。
- [x] 2.4 確認非 UTF-8 行為與 Raw Mirror with PII Scanner 及 Mirror Mask Behavior requirement 對齊。驗證：`cargo test -p codebus-core` 全綠，且 pii-filter 與 vault 相關 scenario 對應的測試皆通過。

## 3. 前端 pattern 數動態化 — Global Settings Modal Field Set（Decision 5）

- [x] 3.1 [P] 落實「Decision 5: 前端 pattern 數由後端真實 builtin 數驅動」的後端來源：在 `codebus-core` 匯出 `pub fn builtin_pattern_count() -> usize`，回傳 `BUILTIN_PATTERNS.len()`。觀察行為：呼叫端取得真實 builtin 數（13）。驗證：新增 `cargo test -p codebus-core` 斷言 `builtin_pattern_count() == 13`。
- [x] 3.2 讓 `codebus-app/src-tauri/src/config.rs` 既有 settings/config IPC payload 帶出該 pattern count（優先擴充既有 command，不新增專用 command）。觀察行為：前端可經既有 config IPC 取得後端真實 pattern count。驗證：`cargo build -p codebus-app-tauri` 通過，且 payload 欄位於 TS 型別/呼叫端可見。
- [x] 3.3 在 `codebus-app/src/App.tsx` 以後端帶出的 pattern count 取代 hardcode 常數 `PII_PATTERN_COUNT = 14`，餵給 `SettingsModal`，取不到時以安全載入佔位 degrade，滿足 Global Settings Modal Field Set 的「PII pattern count is dynamic」scenario。觀察行為：Settings 顯示的 pattern 數等於後端真實 builtin 數、非 UI hardcode。驗證：`npm run typecheck` 通過。
- [x] 3.4 更新前端測試以反映真實 builtin 數與動態來源：將測試中遺留的 `piiPatternCount={14}` 固定值與顯示字串對齊為 13/動態驅動（保留 `renders the runtime PII pattern count, not a hard-coded number` 以 `42` 驗動態的既有測試）。觀察行為：測試不再 hardcode 14。驗證：`npm run test`（含 `SettingsModal.test.tsx`）全綠。

## 4. 收尾驗證與閉集 spec 對齊

- [x] 4.1 跑全套等價驗證。驗證：`cargo test -p codebus-core` 全綠、`cargo clippy -p codebus-core` 無新增警告、`cd codebus-app && npm run test && npm run typecheck` 通過。
- [x] 4.2 閉集 spec 對齊複查：grep pii-filter / vault / app-shell 三 spec 中所有列舉「pattern 集合數量」（如 `four`、`13 patterns`、`14 patterns`）與「非 UTF-8 行為」（`non-UTF-8`、`verbatim`、`BOM`）的 scenario，確認全部同步、無殘留舊數字或舊行為。驗證：grep 結果中不再出現 `14 patterns` 或描述「four PII categories」的舊措辭，且 archive 後逐 requirement grep `^<!-- @trace` count 對齊 requirement 數。
