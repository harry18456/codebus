## Context

`~/.codebus/config.yaml` 的 pii.patterns_extra 來自 app Settings 的「新增 PII 規則」清單。目前該清單允許留空白行，儲存時也不過濾，使一個空字串 pattern 落地。空字串編譯成 regex 是合法的（`Regex::new("")` 成功），但 `find_iter` 會在每個字元位置命中一個 zero-width（start == end）空 match——大型檔案產生數十萬筆 match，raw mirror sync 卡死（實測 CodeCop 241KB markdown，`codebus init` 2 分鐘跑不完）。

同一條「app 儲存設定」路徑的第二個缺陷：save_global_config 以 serde_yaml 重新序列化整份 config，serde 不保留 comment，於是 starter config 手寫的整套教學註解在使用者第一次用 app 改設定後就被洗光。

兩問題同源（皆出在 app 儲存設定路徑），一併修。掃描層是 codebus-core 的 PII 安全地基，屬安全相關。

## Goals / Non-Goals

**Goals:**

- 含空 / zero-width pattern 的 config 跑 init / raw_sync 不再卡（掃描秒級完成、不產生爆量 match）。
- 空 pattern 永不落地（來源端過濾）、且 scanner 對任何 zero-width pattern 都免疫（根本防禦）。
- starter 與 app-saved config 形態一致：共用一段極簡 header + 純值，無 inline 欄位教學註解；教學改放穩定文件。

**Non-Goals:**

- 完整 YAML comment round-trip（明確不做——複雜且脆弱，正是改純值要避開的）。
- 移除 builtin email / ipv4 pattern（另議，本 change 不動 builtin pattern 集，builtin_pattern_count 維持 13）。
- HTTP-based PII scanner。
- 在 load_pii_config 載入層額外過濾空 pattern（評估後判定冗餘，見 Decisions）。

## Decisions

### 層 1：scanner 掃描略過 zero-width match

RegexBasicScanner::scan 的 find_iter 迴圈中，對 start == end 的 match 直接 continue（不 push）。這是根本修法：擋掉任何 zero-width pattern（空字串、`a*`、`\b`、`.*` 在無內容處…）造成的逐字元爆量，不只空字串這一種觸發源。

- 為何不只在 new() 擋空字串：那只解掉「空字串」一種來源，`a*` / `\b` 等可零寬匹配的非空 pattern 仍會爆。scan 層的 zero-width 守門是唯一能涵蓋全部 zero-width 來源的位置。
- 正確性：zero-width match 本就不對應任何實際被遮罩的文字（matched_text 為空），略過它在語意上正確——沒有「漏掉一個真正的命中」。builtin pattern 全部要求至少一個字元 / 固定前綴，無一可零寬匹配，既有測試不受影響。

### 層 2：三處過濾空 / 純空白 patterns_extra

縱深防禦，讓空 pattern 從來源到 scanner 都被清掉：

1. RegexBasicScanner::new()：編譯 patterns_extra 前略過 trim().is_empty() 的條目（不編譯、不成為規則）。先過濾再 enumerate，使 custom-N 編號只計非空且保持連續（既有 custom-0 測試不破）。
2. 後端 save_global_config：寫檔前濾掉 pii.patterns_extra 的空 / 純空白條目，空 pattern 永不寫進磁碟。
3. 前端 settings store save()：送出 IPC payload 前濾掉空 / 純空白 patterns_extra，UI 不送空規則。

- 為何三處而非一處：層 1 已保證「不爆」，但空規則仍是無用噪音、會污染 config 檔與 UI。來源端（前端 + 後端 save）過濾使檔案乾淨；scanner new() 過濾是 scanner 對呼叫者不信任的自我防護。三者各守一個邊界、彼此獨立。

### CONFIG_HEADER 編譯期單一來源（macro_rules!）

定義 `macro_rules! config_header` 展開為一段 2-3 行 header 字面字串；`pub const CONFIG_HEADER: &str = config_header!()`，`STARTER_CONFIG = concat!(config_header!(), <純值 body 字面>)`。app 端 save 從 codebus_core re-export 引用 CONFIG_HEADER。

- 為何用 macro：要讓 starter 與 app-save 真正共用「同一份」header 文字。const 不能餵給 concat!（concat! 只吃字面 token），但 macro 展開成字面可以——達成 compile-time 真單一來源，且 STARTER_CONFIG 維持 const（不改 public API、零既有測試破壞）。
- 替代方案：(a) 兩份字面 + guard test 斷言 STARTER_CONFIG.starts_with(CONFIG_HEADER)——可行但文字有兩份；(b) 改成 `fn starter_config() -> String` 拼接——會改 STARTER_CONFIG 的 const 形態並改動 6 處既有測試。選 macro，churn 最小且真單一來源。仍保留 starts_with guard test 作雙保險。

### starter 改純值、教學移至 docs/config-reference.md

STARTER_CONFIG body 移除所有 inline 欄位 `#` 教學註解，只留實際的 key: value（值不變，round-trip-to-defaults 不變）。原本逐欄教學（pii / agent / hooks / lint / log 各 knob 的說明、azure 範例區塊）整理進新文件 docs/config-reference.md，CONFIG_HEADER 以一行 doc-pointer 指向它。

- 為何：serde 序列化不保留 comment，inline 教學註定被 app save 洗掉——把教學放在會被洗的位置本身就是設計缺陷。改放穩定的獨立文件。
- docs/config-reference.md 屬對外 reference 文件，放 docs/ 頂層符合 repo 慣例（docs/ 頂層放對外文件）。

### save 重貼 header（字串拼接，非 YAML round-trip）

save_global_config 在序列化出 YAML 字串後，prepend CONFIG_HEADER（單純字串前綴拼接），再原子寫檔。使 starter 與 app-saved config 都是「header + 純值」一致形態。header 是 comment，load 時被 YAML parser 忽略，不影響任何 save→load round-trip。

**載入層（load_pii_config）不額外過濾——評估冗餘。** 曾考慮在 load_pii_config 載入層也濾空 patterns_extra。判定冗餘：層 1（scan zero-width skip）+ scanner new() 略過空，已使任何現存含 `['']` 的 config 立即無害（空不成規則、即使成了也不爆）。再加一層 load 過濾不增加實質防護，故不做，避免過度設計。記錄此決策以免 review 時被重複提問。

## Implementation Contract

**Behavior（ship 後可觀察）：**

- 一份 pii.patterns_extra 含空字串（或 `a*` 這類 zero-width pattern）的 config，跑 `codebus init` / raw mirror sync 對大型檔案不再卡住——掃描在合理時間完成、不產生逐字元爆量 match。
- 透過 app Settings 儲存設定後，磁碟上的 config 不含空 patterns_extra 條目。
- starter（首次 init 寫出）與 app 儲存後的 config 皆以同一段 CONFIG_HEADER 起頭、body 為純值、無 inline 欄位教學註解。

**Interface / data shape：**

- RegexBasicScanner::scan 不再回傳 start == end 的 match。
- RegexBasicScanner::new(patterns_extra) 略過 trim().is_empty() 條目；custom-N label 只對非空條目編號（先過濾再 enumerate），第一個非空仍為 custom-0。
- 新增 `pub const CONFIG_HEADER: &str`（codebus-core，global_starter），並從 codebus_core::config re-export。
- save_global_config 寫出的 YAML 以 CONFIG_HEADER 起頭，且 pii.patterns_extra 不含空 / 純空白條目。
- settings store save() 送出前過濾 config.pii.patterns_extra 的空 / 純空白條目。

**Failure modes：**

- 維持既有 fail-loud：malformed regex（無法編譯）仍在 new() 建構時回 Err；空 / 純空白 pattern 視為「略過」而非錯誤（它不是使用者意圖的規則）。
- save_global_config 既有的驗證 / 原子寫入 / 拒絕不變。

**Acceptance criteria：**

- regex_basic 新測試：空 pattern 對大內容 → 0 match（不爆量）；`a*` 對非空內容 → 0 match；new(["".into()]) 後規則數 == builtin（空不成規則）；new(["", real]) 後 real 仍為 custom-0。
- global_starter：STARTER_CONFIG.starts_with(CONFIG_HEADER)；既有 starter_round_trips_to_defaults 與 schema 子字串測試仍綠；新增斷言 body 不含舊 inline 教學字串（例如某段欄位說明）。
- config.rs：save 後 on-disk YAML 以 CONFIG_HEADER 起頭；含 ["", "real"] 的 patterns_extra save→reload 後只剩 ["real"]；既有 save round-trip 測試仍綠。
- settings.test.ts：save() 對含空 patterns 的 config，傳給 IPC 的 payload 已濾空。
- 全套：cargo test -p codebus-core 與 -p codebus-cli 綠、cargo clippy --workspace 無新警告、codebus-app npm test 與 typecheck 綠。

**Pre-apply 校準（apply 第一步先核對，避免 count / 假設漂移）：**

- zero-width skip 對既有 PII 測試的影響：逐一檢視 builtin pattern，全部要求至少一字元或固定前綴（email / ipv4 / jwt / aws… 無一可零寬匹配），故既有正向 / 負向測試不受影響；zero-width 行為由新測試獨立涵蓋。
- save prepend header 對 config round-trip 測試的影響：所有 save→load 測試走 YAML parse（comment 被忽略），不受影響。global_starter writes_when_missing 比對 body == STARTER_CONFIG（自動隨新 const）；config.rs 測試皆 contains / reload 斷言、無精確 byte 比對。若 apply 時發現任何精確 byte / 起頭行斷言，需同步更新。
- custom-N 編號：new() 先過濾空再 enumerate，既有 custom_pattern_triggers_via_patterns_extra（期望 custom-0）不破。
- builtin_pattern_count 維持 13（層 2 只動 patterns_extra）。

**Scope boundaries：**

- In scope：上述 scan / new / save / store 四處行為 + CONFIG_HEADER 單一來源 + starter 純值 + docs/config-reference.md + 對應測試。
- Out of scope：完整 YAML comment round-trip；移除 builtin email / ipv4；HTTP PII scanner；load_pii_config 載入層過濾；SettingsModal.tsx 的「空行標示為待填」UI 美化（純可選 polish）。

## Risks / Trade-offs

- [zero-width skip 誤殺合法命中] → builtin pattern 無一可零寬匹配；使用者自訂 zero-width pattern 匹配空字串本就無意義，略過是正確行為。新測試明確涵蓋正常 pattern 仍正常命中。
- [macro_rules! + concat! 編譯期單一來源較少見，reviewer 不熟] → 加清楚註解說明動機，並保留 starts_with guard test 雙保險。
- [docs/config-reference.md 可能被全域 doc-blocker hook 擋] → 此文件是 CONFIG_HEADER 指向的穩定載體、已於 discuss 與 user 拍板；若被擋，fallback 為改放 README 的 config 段或 docs/ 既有文件並調整 header 指向（不影響核心 PII 修復）。
- [使用者現存 config 仍寫著 ['']] → 層 1 + scanner new() 使其立即無害（無需使用者手動處理）；下次經 app save 會自動清掉空條目。
