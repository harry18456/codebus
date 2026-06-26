## Context

PII scanner（`RegexBasicScanner`）是 raw mirror 去敏化的核心，設計初衷為「高精度、低誤報的小型 regex pack」。目前 `BUILTIN_PATTERNS` 僅 4 條，常見機密形狀不遮；`raw_sync.rs` 對非 UTF-8 檔直接 `fs::copy` byte-identical，把「無法以 UTF-8 解碼」與「無機密」錯誤等同，UTF-16 文字檔因此完全跳過掃描；前端 pattern 數 hardcode 為 14，與真實 builtin 數脫節、且違反 app-shell spec 自己要求的「runtime 動態」scenario。

關鍵約束：
- regex crate（RE2 風格）**無 lookaround**（`(?!...)` 等），pattern 重疊只能以結構/alternation 規避。
- raw mirror 對 clean 檔的「byte-identical」不變量必須維持；不能因 fail-closed 把正常 UTF-16 文字檔逐出 mirror 而傷 wiki 品質。
- 真二進位檔（圖片等）仍須 byte-identical copy。
- pii-filter、vault、app-shell 三份 spec 多處列舉「pattern 集合數量」與「非 UTF-8 行為」，改集合/行為時須全部同步（閉集陷阱）。

## Goals / Non-Goals

**Goals:**

- 擴充 builtin pattern 涵蓋最常外洩的機密形狀，每條低誤報並有 positive + negative 測試。
- 讓含機密的 UTF-16（帶 BOM）文字檔被掃描並遮罩，不再原文進 mirror。
- 讓「有檔沒被掃」可觀察（彙總計數），不再完全靜默。
- 前端 pattern 數由後端真實 builtin 數驅動，消除 hardcode 與 spec 脫節。

**Non-Goals:**

- 不做需要上下文的偵測（變數名、結構線索）——未來 HTTP scanner 範疇。
- 不為 gitignored / oversized 改變既有設計。
- 不新增 single-impl trait 或 0-consumer API。
- 不處理 UTF-16 *無 BOM* 或其他舊式編碼（Latin-1 等）的啟發式偵測——僅以 BOM 為可靠判據（見 Open Questions）。

## Decisions

### Decision 1: 新增九條高精度 builtin 機密 pattern（共 13 條）

在 `BUILTIN_PATTERNS` 既有 4 條後追加 9 條，最終 13 條。每條的 `pattern_name`、severity 與形狀（prose，精確 regex 於實作時依官方格式定）：

- `github-pat`（Critical）：classic token，前綴 `ghp_`/`gho_`/`ghu_`/`ghs_`/`ghr_` + 36 個 base62 字元。
- `github-fine-grained-pat`（Critical）：前綴 `github_pat_` + 82 個 `[0-9A-Za-z_]`。
- `slack-token`（Critical）：前綴 `xoxb-`/`xoxa-`/`xoxp-`/`xoxr-`/`xoxs-` + `-` 分段數字與字母。
- `google-api-key`（Critical）：前綴 `AIza` + 35 個 `[0-9A-Za-z_\-]`。
- `openai-api-key`（Critical）：見 Decision 2（須避開 `sk-ant-`）。
- `stripe-secret-key`（Critical）：前綴 `sk_live_` + 24+ 個 `[0-9A-Za-z]`（測試用合成、非真實 key）。
- `pem-private-key`（Critical）：標頭 `-----BEGIN (RSA |EC |OPENSSH |DSA |PGP )?PRIVATE KEY-----`。
- `jwt`（Warn）：三段 base64url `eyJ...\.eyJ...\.[0-9A-Za-z_\-]+`（header/payload 以 `eyJ` 開頭 + 兩個 `.`）。
- `db-connection-string`（Critical）：`(postgres(ql)?|mysql|mongodb(\+srv)?|redis|amqp)://[^:@/\s]+:[^@/\s]+@`（scheme + user:pass@，密碼段存在才命中）。

理由：皆為帶固定前綴/結構的高熵 token，誤報率低。severity 採二元閉集（Critical/Warn）既有規則：可直接定位的憑證 → Critical；JWT 因可能是非機密的一般 token、且結構較鬆 → Warn。

替代方案：放更寬鬆的「任何 40-char hex」之類泛用形狀——否決，誤報率太高（commit SHA、hash 等遍地皆是），違反 scanner 低誤報原則。

### Decision 2: OpenAI key 以 alternation 規避與 Anthropic key 的重疊（無 lookaround）

OpenAI key 形狀 `sk-...` 會吞掉既有 `sk-ant-...`（Anthropic）。regex crate 無 negative lookahead，無法寫 `sk-(?!ant-)`。改用 alternation：`sk-(?:proj-[A-Za-z0-9_\-]{20,}|[A-Za-z0-9]{20,})`。第二分支 `[A-Za-z0-9]{20,}`（不含 `-`/`_`）遇到 `sk-ant-` 的 `ant` 後立即碰到 `-` 而於 3 字元（<20）處失敗，故不會匹配 Anthropic key；真實 OpenAI classic key（`sk-` + 48 base62）與 `sk-proj-` 變體則正常命中。

驗收：negative 測試以 `sk-ant-api01-...`（Anthropic 形狀）斷言 **不** 產生 `openai-api-key` match（仍只命中 `anthropic-api-key`）。

替代方案：靠 scanner 結果後處理去重——否決，引入額外耦合且遮罩階段已能合併重疊 span，問題只在「detection 不應誤標 provider」，用 alternation 在 pattern 層解決最乾淨。

### Decision 3: 非 UTF-8 改為 UTF-16 BOM decode-scan，無法解碼者維持 verbatim copy

`raw_sync` 讀檔分支改為：
1. 先嘗試 `read_to_string`（UTF-8）→ 成功則照舊掃描。
2. UTF-8 失敗 → 讀原始 bytes，偵測 BOM：`EF BB BF`（UTF-8 BOM）、`FF FE`（UTF-16 LE）、`FE FF`（UTF-16 BE）→ 轉碼為 UTF-8 字串後掃描。
3. 掃描有命中（且須遮罩）→ 寫 UTF-8 遮罩結果到 destination（編碼從 UTF-16 轉為 UTF-8 是可接受的，輸出更利於 agent 閱讀，且替代方案是讓機密原文外洩）。
4. 掃描無命中 → **維持 byte-identical 原始 bytes copy**（保住 clean 檔不變量；我們掃了解碼後的內容、確認乾淨，故仍原樣複製）。
5. BOM 不符 / 轉碼後仍非法 → 真二進位，fall through 到 verbatim copy（見 Decision 4 的計數）。

理由：BOM 是唯一可靠、零誤判的「這是文字、哪種編碼」判據；以它為界既關掉最可能的 UTF-16 機密外洩破口，又不會把圖片誤判成文字。

替代方案：用 `chardet` 之類啟發式猜測編碼——否決，新增依賴 + 不確定性 + 可能把二進位誤當文字而破壞；BOM-only 雖漏掉無 BOM 的 UTF-16，但安全且零誤判（列入 Open Questions）。

### Decision 4: 不可掃描檔的可觀察性用彙總 counter（不做 per-file manifest）

無法解碼掃描而 verbatim copy 的檔案，在 `SyncSummary` 上累加一個彙總計數器（命名如 `unscanned_files`），由既有 summary log 路徑曝光——比照 oversized counter「彙總是 load-bearing 可觀察面」的既有模式。**不** 為每個這類檔寫 per-file manifest。

理由：影像/二進位資產在多數 repo 數量龐大，per-file manifest 會被圖片洗版、訊號被噪音淹沒；彙總 counter 已能讓「有 N 個檔沒被掃」可觀察，符合 scope B 的「不再靜默」要求且不傷可讀性。

替代方案：比照 oversized 寫 `_codebus-unscanned.md` manifest——否決（噪音），但若日後需要可在後續 change 加上限/過濾後再引入。

### Decision 5: 前端 pattern 數由後端真實 builtin 數驅動

`codebus-core` 匯出 builtin pattern 數的單一真實來源（如 `pub fn builtin_pattern_count() -> usize` 回傳 `BUILTIN_PATTERNS.len()`）。`codebus-app` 經既有 settings/config IPC payload 帶出該數（優先擴充既有 command，而非新增），`App.tsx` 改以該值取代 hardcode 常數餵給 `SettingsModal`（其本已是動態 prop 驅動）。spec 範例數字同步更新為真實 builtin 數（13）。

理由：滿足 app-shell 既有「count 必須 runtime 動態、不可 hardcode」scenario，且數字永不再 drift（單一來源 `BUILTIN_PATTERNS.len()`）。

替代方案（fallback）：僅把 `App.tsx` 常數從 14 改成 13——否決為主方案（仍 hardcode、仍違反 app-shell scenario），僅在後端 IPC 接線受阻時作為降級。

## Implementation Contract

**Behavior：**
- `RegexBasicScanner` 預設（無 `patterns_extra`）對含下列任一形狀的內容產生對應 `pattern_name` 與 severity 的 match：上述 13 條。
- raw mirror sync 對帶 BOM 的 UTF-16 文字檔解碼後掃描；含 Critical 機密者在 destination 以 `[REDACTED:<pattern_name>]` 取代後寫出（UTF-8）；無命中者 destination 為原始 bytes byte-identical。
- 真二進位 / 無法解碼檔仍 byte-identical copy，且 `SyncSummary` 的 `unscanned_files` 計數器每遇一個 +1。
- 前端 Settings 顯示的 pattern 數等於後端 `BUILTIN_PATTERNS.len()`（13），非 UI hardcode。

**Interface / data shape：**
- core：`builtin_pattern_count()`（或等義常數）回傳 `usize`，值 = `BUILTIN_PATTERNS.len()`。
- `SyncSummary` 新增 `unscanned_files: usize` 欄位。
- IPC settings/config payload 新增帶出 pattern count 的欄位（命名於 apply 時與既有 payload 慣例對齊）。

**Failure modes：**
- BOM 偵測到但轉碼失敗（截斷的 UTF-16）→ 視為二進位 fall through verbatim copy + `unscanned_files += 1`（不 panic、不中止 sync）。
- IPC count 取得失敗 → 前端以安全預設（如載入中顯示佔位）degrade，不阻塞 Settings 開啟。

**Acceptance criteria：**
- 13 條 pattern 各有 1 positive + 1 negative 測試；OpenAI/Anthropic 不互吞的 negative 測試存在並通過。
- UTF-16+BOM 含機密檔被遮罩的測試、真二進位仍 verbatim copy 的測試（既有 `mask_mode_falls_through_to_copy_for_non_utf8` 對應更新）皆通過。
- `cargo test -p codebus-core` 全綠、`cargo clippy -p codebus-core` 無新增警告、`npm run test` + `npm run typecheck` 通過。
- pii-filter / vault / app-shell 三 spec 所有列舉 pattern 數量與非 UTF-8 行為的 scenario 同步（archive 前 grep 對齊）。

**Scope boundaries：**
- In scope：`BUILTIN_PATTERNS` 擴充、`raw_sync` 非 UTF-8 分支、`SyncSummary` 計數器、core count 匯出、IPC 帶出、`App.tsx` 接線、三 spec delta。
- Out of scope：SEC-1/SEC-2、gitignored manifest、oversized 重做、無 BOM 編碼啟發式、HTTP-based contextual scanner。

## Risks / Trade-offs

- [新 pattern 誤遮正常程式碼] → 每條強制 negative 測試；形狀皆帶固定前綴/高熵結構，誤報面小。
- [UTF-16 無 BOM 仍漏掉] → 接受為已知限制（BOM-only 換取零誤判），列 Open Questions；多數 Windows 匯出的 UTF-16 帶 BOM，主要破口已關。
- [UTF-16 命中後輸出轉為 UTF-8 改變編碼] → 可接受（mirror 供 agent 閱讀，UTF-8 更佳；clean 檔仍保原 bytes，僅命中檔轉碼）。
- [彙總 counter 不夠細] → 接受（避免 per-file manifest 被二進位洗版）；後續可加 manifest with cap。
- [閉集 spec 漏改某個 scenario] → archive/實作前對三 spec grep `four`/`14 patterns`/`non-UTF-8`/`verbatim` 全列舉點逐一核對。

## Migration Plan

純加性變更，無資料遷移：新 pattern 只增加遮罩面、新 counter 預設 0、前端數字改由後端驅動。Rollback = revert commit 即可恢復舊行為（無持久化 schema 變更於使用者磁碟）。

## Open Questions

- 是否在後續 change 處理「UTF-16 無 BOM」與其他舊式編碼（Latin-1 等）的偵測？本輪僅 BOM。
- `unscanned_files` 是否需要在 GUI 也曝光（目前僅 summary log）？預設否，待使用回饋。
