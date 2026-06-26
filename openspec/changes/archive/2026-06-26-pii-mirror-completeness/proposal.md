## Problem

codebus 的安全核心是「agent 只讀 PII 去敏化後的 raw mirror、不讀 live repo」。但去敏化掃描目前不完整，真實機密仍可能原文進入 mirror 並被 agent 讀到：

1. **內建 pattern 太少**：`RegexBasicScanner` 的 `BUILTIN_PATTERNS` 只有 4 條（`aws-access-key`、`anthropic-api-key`、`email`、`ipv4`）。GitHub PAT、Slack token、Google API key、OpenAI key、Stripe secret、PEM 私鑰、JWT、含密碼的 DB 連線字串等最常外洩的機密形狀完全不遮。
2. **非 UTF-8 檔靜默 verbatim copy**：`raw_sync.rs` 對非 UTF-8 檔（含 UTF-16 文字檔）回傳空 match、直接 `fs::copy` byte-identical，不掃、不警、不遮、不計數。一個 UTF-16 編碼、內含機密的文字檔會原文進 mirror 而完全無痕跡。此行為已被 spec 化（pii-filter「Mirror Mask Behavior」非 UTF-8 scenario、vault「Raw Mirror with PII Scanner」requirement）。
3. **UI pattern 數脫節**：前端 `App.tsx` hardcode `PII_PATTERN_COUNT = 14`、app-shell spec 範例寫 `regex_basic · 14 patterns`，但真實 builtin 只 4 條；且 app-shell spec 已要求該數字「runtime 動態、不可 hardcode」，現況既與真實數脫節、又違反自己的 spec scenario。

## Root Cause

PII scanner 設計初衷是「高精度、低誤報的小型 regex pack」，初版只放了最保守的 4 條，未隨常見機密形狀擴充。非 UTF-8 分支當初以「regex scanner 對非 UTF-8 不產生 match」為由直接 fall through 到 verbatim copy，把「無法以 UTF-8 解碼」與「無機密」錯誤地等同；UTF-16 文字檔因此被歸入二進位處理而完全跳過掃描。UI 數字則自始 hardcode、未接後端真實 pattern 數。

## Proposed Solution

- **擴充 `BUILTIN_PATTERNS`**：加入高精度、低誤報的機密形狀（GitHub classic / fine-grained PAT、Slack token、Google API key、OpenAI key、Stripe secret key、PEM 私鑰標頭、JWT、含密碼的 DB 連線字串），每條搭配 1 個 positive（真機密命中正確 severity）+ 1 個 negative（近似的正常字串/程式碼不誤遮）測試。OpenAI key 形狀須避免吞掉既有 `sk-ant-`（Anthropic）——regex crate 無 lookaround，改以 alternation 規避（詳 design.md）。
- **非 UTF-8 不再靜默**：偵測 UTF-16 LE/BE BOM 的文字檔 → 轉碼為 UTF-8 後掃描（命中則遮罩、輸出 UTF-8；無命中則維持 byte-identical 原始 bytes）；真正無法解碼的二進位檔仍 byte-identical copy，但累加一個 summary 計數器（彙總可觀察），不再完全靜默。取捨（per-file manifest vs 彙總 counter）於 design.md 定案。
- **對齊 pattern 數**：讓 `App.tsx` 的 pattern 數由後端真實 builtin 數驅動（消除 hardcode，滿足 app-shell 既有「runtime 動態」scenario），spec 範例數字同步更新為真實 builtin 數。

## Non-Goals

- 不碰 SEC-1（agent spawn env scrub）、SEC-2（codex hard read）——各自獨立 change。
- 不為 gitignored 檔做 manifest——gitignored 不進 mirror 是正確的安全設計，本輪不做。
- 不重做 oversized（>5 MiB）manifest——該機制已完成。
- 不新增抽象層（single-impl trait / 0-consumer API）；直接擴充現有 `BUILTIN_PATTERNS` 與既有 `raw_sync` 分支。
- 不引入需要上下文（變數名、結構線索）的偵測——那屬未來 HTTP-based scanner 範疇，仍 out of scope。

## Success Criteria

- 每個新增 pattern 都有 1 positive + 1 negative 測試，且 negative 證明不誤遮常見正常字串/程式碼。
- OpenAI key pattern 不會把 `sk-ant-...`（Anthropic key）重複/錯誤標記成 OpenAI（有 negative 測試守住）。
- 一個含機密的 UTF-16（帶 BOM）文字檔經 sync 後，destination 中該機密已被 `[REDACTED:<pattern_name>]` 取代（不再原文外洩）。
- 真二進位檔（如圖片）仍 byte-identical copy，且無法掃描的檔案數可由 summary 計數器觀察。
- 前端顯示的 pattern 數等於後端真實 builtin pattern 數，且該數非 hardcode（由後端驅動）。
- pii-filter / vault / app-shell 三份 spec 中所有列舉「pattern 集合數量」與「非 UTF-8 行為」的 scenario 全部同步一致。
- `cargo test -p codebus-core` 全綠、`cargo clippy -p codebus-core` 無新增警告、`npm run test` 與 `npm run typecheck` 通過。

## Impact

- Affected specs: pii-filter（modified）、vault（modified）、app-shell（modified）
- Affected code:
  - Modified:
    - codebus-core/src/pii/scanners/regex_basic.rs
    - codebus-core/src/vault/raw_sync.rs
    - codebus-core/src/vault/manifest.rs
    - codebus-app/src-tauri/src/config.rs
    - codebus-app/src/App.tsx
  - New: (none — 直接擴充既有檔案；新增測試置於既有測試模組/檔案內)
  - Removed: (none)
