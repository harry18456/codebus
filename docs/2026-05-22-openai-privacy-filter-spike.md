# T13 Spike：OpenAI Privacy Filter 整合（語意層 PII）

**Date:** 2026-05-22
**Task:** loop T13（只讀探勘）
**背景:** [openai-privacy-filter backlog](2026-05-14-openai-privacy-filter-backlog.md)（2026-05-14, parked）

---

## TL;DR

backlog 技術細節完整。**對現碼核對找到 3 個有用的更新**:
1. **依賴 `pii-settings-ui` 已 archive**(2026-05-20, `settings-config-frontend`)——backlog `:96` 寫「可同批做」失效,UI 已存在,SemanticPiiScanner toggle 只需在現有 Settings PII 區塊**追加**一條。
2. **必須先修 F1** ([T6 PII mask 重疊未合併](2026-05-22-core-quality-review.md)):**semantic + regex 兩層共存,重疊 match 必然更頻繁**(semantic 的人名 span 常蓋過 regex 的 email/IP),F1 不修 → mask 直接踩雷。順序依賴。
3. **ONNX runtime 選擇要跟 T12 對齊**([T12 spike O1](2026-05-22-rag-index-search-spike.md))——兩條 backlog 都想用 ONNX,先動的人定基礎設施(crate 選擇、binary 打包、跨平台 build)。

---

## 現況核對

1. **零 ONNX/ort deps**(T12 已查證)。net-new heavy dep。
2. **`RegexBasicScanner` + on_hit 設施完整**(`pii/scanners/regex_basic.rs` + `vault/raw_sync.rs`),`SemanticPiiScanner` 只需新增第三 scanner、走同個 trait;**兩層共存** backlog `:50` 設計 → 結果同樣丟進 `mask_matches` → 觸發 F1 路徑放大。
3. **Settings UI PII 區塊已存在**(`pii-settings-ui` 2026-05-20 archive 進 settings-config-frontend),`SemanticPiiScanner toggle` 加在那個區塊即可,不必另建。

## 對 backlog 的補充

### O1：先修 F1 是 hard prerequisite

backlog `:43-50` 設計兩層 scanner 結果合併。F1 證明現有 `mask_matches` 不處理重疊。**Semantic + regex 同檔同位置兩層命中是常態**(name span 在 email 周圍、address 含 IP 等)。**若先 ship semantic、後修 F1,線上馬上有 mask 損壞 / PII 漏遮**。
→ 寫進實作 tasks: 「Task 0: 補 mask_matches interval-merge(F1)」放第一步。半天額外工程,但解決 baseline 正確性。

### O2：與 T12 共用 ONNX runtime 基礎設施

backlog `:94` 自己提了。記在這側雙向 cross-link:**誰先動誰定 crate**(`ort` 看起來 backlog `:57` 預設,T12 沒指定;先動者拍板)。`tract`(純 Rust、無 native lib)vs `ort`(bind 官方 onnxruntime、效能好但 native lib distribution 麻煩)是主要 trade-off,值得單獨 0.5 天 spike。

### O3：backlog `:96` 「可同批做 pii-settings-ui」需更新

該條 backlog 已 archive(`settings-config-frontend` Change 1),UI 已落地。實作時只在現有 PII Settings 區追加 toggle + 「首次下載模型」進度條,不需獨立 UI 工作量。

## 順序建議（若哪天起 propose）

1. **F1 修(半天)** — 先,讓 mask 對重疊安全。
2. ONNX runtime 選擇 spike(0.5 天)— 與 T12 對齊。
3. SemanticPiiScanner + 模型下載 / cache(2-3 半天)— backlog tasks 3-4。
4. config schema + Settings toggle(0.5 天)— UI 已在,順手加。
5. 整合測試(0.5 天)— 含重疊覆蓋場景。

backlog 原估「3-5 個半天」基本對,加 F1 預先修約多 0.5 天。

## 待 harry
無新阻塞。此條本就低優先(優先序低於 F);**重要的是萬一哪天動,F1 必須先修**——已記入順序建議,實作時別忘了。
