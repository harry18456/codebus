# Design Audit

> 對照 `design-handoff/README.md` 規格與 `design_files/` mock，逐畫面盤點現況 gap。
>
> **流程**：chat push back / 同意後才寫進來。
>
> **不在這裡寫**：實作方案、優先序、估時。**結算階段（見文末 `## 結算階段 · Next Steps`）才寫**。

## Design v1 Reply Status（2026-05-26）

Design team 已 reply v1 audit。Sources（皆在本目錄）：
- `design-reply.html` — 11 cross-cutting calls + 10 Open Q inline answers + visual demos
- `walkthrough-decisions.html` — **取代三場 walkthrough**，把 CJK labels / 02 activity stream / Quiz scope-confirm 三段都寫到「能直接開工」細度（含 token spec、14 位置 sweep、Shell kind 3 層、Design A/B 雙版方案、state machine 等）

**整體結果**：11 cross-cutting 10 ship / 1 pushback（R7-1 keep 💬 已採）。10 Q 全採。Walkthrough 三場**全部 async 答完、不用排**。

**Harry 採納決議**（all in，2026-05-26）：

- **A. R7-1 💬**：keep 💬（不改 MessageSquare / 🚌）；ChatWidget 圓鈕 emoji 不動，ambient signal 走 ODI-4 amber 7px pulse dot（200ms fade-in、跟 stream tail 同步）
- **B. 02 activity stream**：採 2-phase cluster + Shell kind 3 層 enum（`read` / `inspect` / `mutation`）
- **C. CollapsibleStreamLog**：per-user sticky via localStorage（key `codebus.stream-log.default-open`）
- **D. 其他 9 Qs 全採**（02c rename、quiz fail=red、wizard CTA hidden、scope-confirm flagship 等）

**8 個 ⚠ 對齊點決議**（2026-05-26 全採我建議）：

| # | 議題 | 決議 |
|---|---|---|
| 1 | Wiki taxonomy 英還是中？ | **保留英文 caps**（跟 README 5-bucket / Karpathy taxonomy / CLI 對齊），走 `.section-label--caps` 變體 |
| 2 | amber bar 高度 | **12px**（design default），實機若太矮再調 14 |
| 3 | Shell kind 由 skill 還是 core？ | **skill 自己標**（skill 知 intent；core default 太粗） |
| 4 | 不在表內 tool（Task/WebFetch）default？ | **READING CODEBASE** + 擴 kind enum 加 `"other-read"` / `"other-write"`，不新開 cluster 欄位 |
| 5 | 02b cluster summary 文案中英？ | **中文化**（跟 i18n 原則一致；ambient mono 仍 OK）|
| 6 | Quiz scope-confirm Design A vs B | **走 A**（with match metadata）——trust-builder 厚度值得；codebus-quiz skill output schema 加 reason field 工程量小 |
| 7 | 「重新規劃」LLM context | **接受**——`codebus-quiz` skill prompt 加 `previous_rejected_paths: string[]` input |
| 8 | Topic banner emoji 🎓 vs 🎯 | **🎓**（Quiz 自己一致；🎯 留給 Goal） |

**已 cancel** 三場 walkthrough（design 提的 30/20/45min）——`walkthrough-decisions.html` 已寫完所有 spec、所有對齊點 async 答完。

**等 v1.1 handoff**（design 下週末交付）：LoadingOverlay (live progress) / 02c Interrupted 正式 spec / Quiz wizard 完整 4 步（topic/generating/completion mock）/ Wiki page reader / ChatWidget centered-modal mode 規格。

詳細 absorb 後的決策見下方對應條目。

---

## Tag Legend

| Tag | 意思 |
|---|---|
| `[local]` | 該畫面特有問題 |
| `[shared]` / `[shared with X]` | 跨畫面共用問題；指向其他 gap ID |
| `[bug]` | 行為錯誤、需獨立修 |
| `[verify]` | 觀察項目尚未確認，結算階段實機驗 |
| `[open]` | 決策未定，待後續對齊 |
| `[defer]` | 已知問題但決定暫不處理 |
| `[skip]` | 不會做（設計灰區、價值不夠） |
| `[confirmed]` | 已驗過（用在原本 `[verify]` 升級後） |
| `[by design]` | 看似 gap 但實際是設計意圖 |
| `[observation]` | 純觀察，未升級成 gap |
| `[design 灰區]` | spec 沒覆蓋、codebus 自加實作 |
| `[non-audit-scope]` | 超出 design audit 範圍但要記錄 |
| `[i18n Cat A/B/C/D]` | i18n 補洞分類，詳見 `## Cross-cutting · i18n` |
| `[high priority]` / `[low priority]` | 結算階段排序提示 |
| `[partial]` | 部分修法已定、其他待結算 |
| `[updated YYYY-MM-DD]` | 此條目曾被修正、註記時間 |
| `[→ X]` | 純 cross-reference stub、見指向條目主體 |

Review 進度：

- [x] 04 Lobby（04a populated / 04b empty）— 完成 2026-05-25
- [x] 01 Vault Workspace（sidebar / empty / populated）— 完成 2026-05-25
- [x] 02 Goal Detail（02a running / 02b completed）— 完成 2026-05-25
- [x] 03 Quiz（03a pending / 03b reviewing + codebus 自加 wizard flow / completion / list / review-all）— 完成 2026-05-26
- [x] ~~05 Cmd+K Overlay~~ — 決定砍掉 2026-05-25（理由見下方 05 區）
- [x] 06 Settings Modal — 完成 2026-05-25
- [x] **Wiki tab（design 灰區，codebus 自加實作）** — 完成 2026-05-26

---

## Cross-cutting · i18n

> 確認時間 2026-05-25。完整 sweep 跨 41 個 component 檔，盤點所有 hard-code English UI text。
>
> 架構面 i18n pipeline **本身乾淨**（`i18n/messages.ts` TS-typed bundle、`useT` hook、`errors.ts` LocalizedError、`Toast.tsx` locale-aware）；**問題只在組件層沒用**——大量 JSX hard-code 英文字串，zh-tw 環境下永遠英文，跟周圍中文 UI 混雜成「亂」感。

### 原則（harry 確認 2026-05-25）

1. **所有 user-facing text 都要走 i18n**——含 button label、placeholder、aria-label、title attr、DialogTitle、error message、status label
2. **例外（保留英文當 jargon brand）**（2026-05-25 擴充自 06 Settings 審查）：
   - **Tab labels**：Workspace 三個 `Goals` / `Wiki` / `Quiz`（codebus 核心動詞、CLI 也是這幾個字）
   - **Verb names**：`goal` / `query` / `fix` / `verify` / `chat`（CLI verb，跟 `~/.codebus/config.yaml` 對齊）
   - **Codex effort 值**：`low` / `medium` / `high` / `xhigh`（codex API 固定 enum）
   - **PII 命中處理值**：`warn` / `mask` / `block` 等（PII 行為 enum）
   - **Config YAML key 名**：`base_url` / `api_version` / `keyring_service`（跟 yaml 對齊）
   - 共同理由：這些是 user 必須學的領域術語、跟 CLI/config.yaml/API 對齊；翻成中文反而破壞 mapping
3. **不在範圍**：log/comment 內的英文（不渲染給 user）、tool name identifier（`Read` / `Write` / `Glob` / `Grep` / `Edit` 等是 Claude API tool name，是 identifier 不是 UI label）

### Gap 清單

#### Cat A · Settings panel 完全沒 i18n [shared]

| 檔案 | 待處理 |
|---|---|
| `settings/EndpointSection.tsx` | section heading「Claude Code endpoint settings」、`label="System"` / `label="Azure"`、card title「System Profile」/「Azure Profile」、`label="API key"`、status「Set / Unset」、`aria-label="Active endpoint profile"`、placeholders（`<model, e.g. opus-4-7>`、Azure base_url、deployment name） |
| `settings/CodexEndpointSection.tsx` | 同上 pattern，外加「OpenAI Codex endpoint settings」、「Endpoint configuration is incomplete:」、`label="effort"` |
| `settings/SetKeyDialog.tsx` | DialogTitle「Set Azure API key」、error「API key cannot be empty」、button「Confirm」/「Saving…」 |

#### Cat B · Workspace 零星 hard-code [shared]

| 檔案 | 待處理 |
|---|---|
| `workspace/QuizAnswering.tsx` | 「Passed/Failed (threshold {n}%)」、「Correct/Incorrect」、「Finish/Next」 |
| `workspace/QuizReview.tsx` | 「Passed/Failed (threshold {n}%)」、DialogTitle「Generation log」 |
| `workspace/QuizTab.tsx` | DialogTitle「Generation log」、error「Quiz failed: {errorMsg}」、placeholder「What do you want to be quizzed on?」 |
| `workspace/NewGoalModal.tsx` | DialogTitle「New goal」、placeholder「What should codebus document?」 |
| `workspace/ChatInput.tsx` | placeholder「Type your message...」 |

#### Cat C · 共用 UI 元件的 aria-label / title attr（a11y 也跟著漏） [shared]

| 檔案 | 待處理 |
|---|---|
| `ui/dialog.tsx` | `aria-label="Close"` |
| `workspace/ChatWidget.tsx` | `aria-label="Open chat"` / `"Resize chat widget"` / `"Minimize chat"`、`title="Drag to resize"` |
| `workspace/ChatTranscript.tsx` + `ExplanationText.tsx` + `WikiPreview.tsx` | 3 處 `title="Page not found"`（建議抽到 shared key） |
| `workspace/WikiTab.tsx` | `aria-label="Toggle Pages tree"` |

#### Cat D · 保留英文（design decision，不是 bug） [local]

> 2026-05-26 擴充：跟原則 #2 同步，列出所有保留英文當 jargon 的範疇。

| 類別 | 保留字串 | 出現位置 | 理由 |
|---|---|---|---|
| Workspace tab labels | `Goals` / `Wiki` / `Quiz` | `workspace/Workspace.tsx:272/282/288` | codebus 核心動詞、CLI 也是這幾個字 |
| Verb names | `goal` / `query` / `fix` / `verify` / `chat` | `settings/EndpointSection.tsx` / `CodexEndpointSection.tsx` per-verb model rows | CLI verb，跟 `~/.codebus/config.yaml` 對齊 |
| Codex effort 值 | `low` / `medium` / `high` / `xhigh` | `settings/CodexEndpointSection.tsx` effort dropdowns | codex API 固定 enum |
| PII 命中處理 enum | `warn` / `mask` / `block` | `SettingsModal.tsx` PII 命中處理 dropdown | PII 行為 enum |
| Config YAML key 名 | `base_url` / `api_version` / `keyring_service` | `settings/EndpointSection.tsx` / `CodexEndpointSection.tsx` field labels | 跟 `~/.codebus/config.yaml` key 對齊 |
| Tool name identifiers | `Read` / `Write` / `Glob` / `Grep` / `Edit` / `Bash` 等 | `RunDetail*.tsx` switch cases / `ActivityStreamItem.tsx` | Claude API tool name，是 identifier、不是 UI label |

**實作建議**：Cat D 也走 i18n bundle（en/zh 兩個 locale 都填英文），方便未來統一改動，避免散落 hard-code。

### 不在範圍

- `RunDetail*.tsx` 的 tool name 比對（`case "Read":`、`case "Write":` 等）——是 identifier 而非 UI label
- comment / JSDoc 內的英文
- 已經走 i18n 的部分（Lobby empty/populated、VaultCard、common、errors、Toast）

### 結算階段要做的事

1. 把所有 Cat A/B/C 字串加進 `i18n/messages.ts` 兩種 locale
2. 把組件改用 `t("...")`
3. Cat D 也走 i18n bundle（en/zh 都填英文）方便未來改
4. 補測：vitest 跑一遍確認沒 break；切到 zh locale 手動 smoke 一輪 Settings + Quiz + Chat 確認沒漏網
5. 翻譯 wording 細節（design v1 reply 已 confirm）：
   - 「Endpoint configuration is incomplete:」→ 「端點設定不完整：」（`端點` 翻中文；jargon 限 `base_url` / `api_version` key names 本身）
   - aria-label 抽 **shared i18n key per concept**——例如 3 處 `title="Page not found"` 統一 `a11y.pageNotFound` 一個 key、不要三個分別 key

---

## 04 · Lobby

### 04b · Empty

- **現況截圖**：`codebus-app/scripts/.lobby-current.png`（2026-05-25 透過 CDP 抓 real WebView2）
- **規格來源**：
  - `design-handoff/README.md` § `04a / 04b · Lobby`
  - `design-handoff/design_files/components/lobby.jsx`
  - `design-handoff/design_files/styles.css`（`.cb-empty-*`、`.cb-quickstart-*`、`.cb-lobby-foot`）
- **現況實作**：
  - `codebus-app/src/components/lobby/Lobby.tsx`
  - `codebus-app/src/components/lobby/EmptyState.tsx`

#### Gap

##### G-copy-1 · 副標與 Quickstart 步驟去 jargon [local]
- **問題**：現況副標「選一個 repo、跑一個 goal，先讓 codebus 帶你看懂這份程式碼。」+ Quickstart step1/step2 同樣用 `repo` / `goal` jargon
- **不對在哪**：
  - `repo` 過度具體：codebus 跑的是任何程式碼資料夾，不限 git
  - `goal` 是 codebus 內部術語，新使用者第一次進 lobby 完全不知道是什麼
- **新版（確認 2026-05-25）**：
  - `lobby.empty.subtitle` → 「指一份程式碼資料夾、想一個想搞懂的問題，codebus 邊讀邊幫你做筆記。」
  - `lobby.empty.step1` → 「選一份程式碼資料夾」
  - `lobby.empty.step2` → 「想一個想搞懂的問題（goal）— 例如『auth 怎麼運作』」
  - `lobby.empty.step3` → 不動
  - 標題「來搭第一台公車吧」不動（標題保留 brand 鉤子、副標負責白話解釋，分工明確）
- **原則**：保留 instructional 路線、只換詞；`goal` 在 step2 以括號 hint 形式介紹，作為進 Workspace 看到 `Goals` tab 前的學習觸點
- **連帶要動的英文版**（`messages.ts:37-44`）：保持對應結構，副標換成「Point at a code folder, pick a question to dig into — codebus reads it and takes notes for you.」step2 換成「Pick a goal — e.g. "how does auth work?"」（待最終確認）

##### G1 · 內容垂直置中導致上下大留白 [local]
- **問題**：`Lobby.tsx:49` `items-center justify-center` 讓 hero 落在 viewport 幾何中心；設計稿是內容自然向上排、`cb-lobby-foot` 黏底
- **影響**：04b 「沒做完」感最主要來源
- **修法方向**（結算階段定細節）：layout 改成 `flex flex-col` + 內容上中、footer `mt-auto` 黏底；hero 跟 footer 之間的距離由內容自然撐開
- **附驗收**：修完在常用 fullscreen 尺寸再看一次；若仍空再考慮升級成 ODI-2（背景 ambient）

##### G2 · Quickstart step2 缺 amber quote pill [local]
- **問題**：step2 example 是純 text 配中文引號；設計稿是 inline amber-tinted mono pill（`cb-qs-quote mono` + `accent-tint` 底）
- **影響**：Quickstart card 唯一視覺重點缺失、整張卡無 accent
- **連動 G-copy-1**：pill 包的內容會是新文案「auth 怎麼運作」（不是「搞懂這 repo 的 X」）
- **修法方向**：JSX 內把 example 抽成 `<span>` 套用 mono + bg-accent-tint + text-accent + rounded-sm + 細 padding；i18n key 用 `{example}` placeholder 注入

##### G3 · 步驟編號帶句點 `1.` `2.` `3.` [local]
- **問題**：`EmptyState.tsx:48` 用 `{i + 1}.`；設計稿是 `cb-qs-num mono` 純 mono 數字、無句點、dim 色
- **影響**：「鬆散感」來源之一
- **修法方向**：去句點、改 mono 字體、配 `text-fg-tertiary`

##### G4 · 中文 section label 視覺替代方案 [shared] [design v1.5 spec lock]
- **問題**：套了 `uppercase tracking-[0.12em]` 但中文「快速開始」沒 uppercase 概念，視覺僅剩 10px 粗體；「近期 VAULT」 中英混雜時 tracking 只對 Latin fragment 生效、看起來像 bug
- **Token spec**（walkthrough-decisions.html §01.1 lock 2026-05-26）：

```css
/* 通用 treatment — 14 位置裡 11 個用這個 */
.section-label {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  font-size: 12px;          /* 字號 +1 級後對齊新的 meta 12 */
  font-weight: 600;
  color: var(--fg-secondary);
  letter-spacing: 0;        /* 不再 tracked */
  text-transform: none;     /* 不再 uppercase */
}
.section-label::before {
  content: "";
  width: 2px;
  height: 12px;             /* 對齊點 2：實機若太矮再調 14 */
  background: var(--accent);
  border-radius: 1px;
}

/* English taxonomy 變體 — 全 app 只用於 Wiki tree 5 buckets */
.section-label--caps {
  text-transform: uppercase;
  letter-spacing: 0.08em;   /* 從原 0.12em 收斂 */
  font-size: 11px;
  color: var(--fg-tertiary);
}

/* 右側 mono count（pattern 跟 sidebar nav count 一致） */
.section-label__count {
  margin-left: auto;
  font-family: 'JetBrains Mono', monospace;
  font-size: 11px;
  color: var(--fg-tertiary);
}
```

- **Retire 原 `cb-section-label` uppercase-tracked token**
- **14 位置 sweep**（詳見 `walkthrough-decisions.html` §1.2 完整 table）：
  - **drop entirely**（label 拿掉）：Workspace sidebar VAULT（單一 group）/ Wiki tree OTHER bucket
  - **`.section-label` 通用版**：Lobby populated「最近」、Lobby empty「快速開始」、Goals「最近」、Goal Detail（COVERED→「Wiki 變更」、LINT、活動摘要、注意事項、READING/WRITING phase）、02c「目前進度」、Quiz history「最近/上週/更早」、Settings 各子段
  - **`.section-label--caps` 英文 caps 變體**：Wiki tree 5 buckets（MODULES / PROCESSES / SYNTHESIS / CONCEPTS / ENTITIES）—— **對齊點 1 決議：保留英文**
  - **不用 section-label**：Quiz wizard step header（走 dots + label）、LoadingOverlay 子標（走 status line treatment）
- **Edge cases**（walkthrough-decisions §1.4）：
  - **子段（second-level）不用 amber bar**——避免同層出現兩支 bar 切碎畫面；改用 `font-size: 13px; font-weight: 500; color: var(--fg);` + 上方 8px gap
  - **section + count + action 同 row**：label+count 一起左對齊（label 內部 `margin-left: auto`），action button 獨立靠右；不要塞進 label
- **連動 13 處**：S3 / GP3 / D1 / D4（COVERED PAGES）/ I3 / WK1 / WK7 / QC2 跨多畫面套同一 token

##### G5 · Topbar / content 分隔線太淡 [shared] [design v1 ack]
- **問題**：`Lobby.tsx:70` `border-b border-border`（`#1f1f1f` on `#0a0a0a`），實機完全看不到線
- **設計稿**：1px hairline 應該肉眼可見
- **決策**（design v1 確認 2026-05-26）：
  - **Promote default `--border` 從 `#1f1f1f` → `#2a2a2a`**（即原 `--border-strong`）
  - **保留** `#1f1f1f` 但**限定用在 in-card row separators**（要 「almost invisible」 的場合）
  - 設計稿是 Figma 純黑底量、實際 ClearType sub-pixel anti-aliasing 會吞線；不是 calibration、是真感知 floor
- **影響範圍**：所有用 `border-border` 做分隔線都受惠——topbar 底、footer 頂、card border、column separator、goal table row 等

##### G6 · Footer 缺頂線 + sunken bg [local]
- **問題**：⚙️ 設定 / v3.0.0 浮在底，沒 footer 區塊感
- **設計稿**（已 verify `design_files/styles.css` `.cb-lobby-foot`）：
  - `flex: 0 0 32px`（固定 32px 高）
  - `border-top: 1px solid var(--border)`
  - `background: var(--bg-sunken)` (`#070707`)
  - `padding: 0 12px`
  - `font-size: 11px`
- **修法**：依上述 spec 套 token；連動 G5（border 對比度）統一處理

##### G7 · Quickstart card padding 偏大、行距偏鬆 [local]
- **問題**：`EmptyState.tsx:35` `p-[14px_18px]` 看似合規，但 `ol.space-y-2` + `mt-2` 累加後 card 內部偏鬆
- **設計稿**：14/18 padding + 較緊步驟間距
- **連動 G3**：步驟編號樣式修完視覺密度會差很多，G7 跟 G3 一起調更合理

### 04a · Populated

- **現況截圖**：`codebus-app/scripts/.lobby-after-add.png`（2026-05-25，加一個測試 vault 後拍）
- **規格來源**：
  - `design-handoff/README.md` § `04a / 04b · Lobby`（Topbar / Populated / lobby foot）
  - `design-handoff/design_files/components/lobby.jsx`
- **現況實作**：
  - `codebus-app/src/components/lobby/Lobby.tsx`（Topbar、PopulatedList）
  - `codebus-app/src/components/lobby/VaultCard.tsx`

#### Gap

##### [shared] 跟 04b 共用問題（這次得到實證）
- **G1 內容垂直置中** — 04a 一張 vault card 後上方仍有大塊空白
- **G4 中文 section label** — 04a 「近期 VAULT」中英混雜（uppercase tracking 對「近期」沒效、對「VAULT」有效），比 04b「快速開始」**更刺眼**；修法跟 04b 同套替代視覺
- **G5 topbar 分隔線太淡** — 一樣看不到
- **G6 footer 缺頂線 + sunken bg** — 一樣浮在底
- **G7 密度** — vault card 內 name 跟「上次開啟 剛剛」間 padding 偏鬆

##### G-copy-2 · 移除 UI 層 "vault" 詞 [local]
- **問題**：UI 層出現「Vault」三處且寫法不一致（首大寫 / 全大寫 / 全小寫）
  - topbar：`+ 新增 Vault`
  - section label：`近期 VAULT`
  - drag tip：`...就能開啟成新 vault。`
- **背景**：`vault` 是 Obsidian 的詞（codebus 生成 Obsidian-compatible wiki 所以沿用）。但多數 user 不是 Obsidian 重度使用者；UI 已經用「程式碼資料夾」介紹（副標 G-copy-1），同概念兩名稱更糟
- **與 D1 例外的差異**：Goals/Wiki/Quiz 是 user 反覆觸發的動詞（保留英文 brand 合理），vault 只是容器名詞、user 觸發少，不必走 jargon brand
- **決策**（harry 確認 2026-05-25）：**UI 層完全拿掉 vault 詞**；CLI / README / 內部 store（`VaultEntry` 等）不動
- **新詞表**：
  - `lobby.topbar.newVaultButton` → 「+ 新增」（en: `+ Add`）
  - `lobby.populated.sectionLabel` → 「最近」（en: `Recent`）
  - `lobby.populated.dragTip` → 「提示・把資料夾拖進這個視窗就能加入清單。」（en: `tip · Drag a folder anywhere into this window to add it to the list.`）
- **附帶解決**：原 G-04a-1（三處寫法不一致）自動消失；副標「程式碼資料夾」vs topbar「Vault」概念脫鉤也自動解決
- **i18n key 命名不動**：`newVaultButton` / `populated.sectionLabel` 等 key 保留，只改 value——key 是技術 identifier、不是 user-facing

##### G-04a-1 · Vault card 互動模型 — kebab discoverability [local]
- **問題**：設計稿是 hover-revealed `⋮` kebab 按鈕 → click 開 menu；現況改用 right-click context menu（`VaultCard.tsx:37` onContextMenu）、**沒可見 entry point**
- **影響**：新 user 不知道有 Reveal in files / Remove 選項
- **修法方向**（結算階段定細節）：補可見 kebab（hover 顯示）；保留 right-click 當 shortcut
- **i18n 連動**：menu 項目 `vaultCard.menu.revealInFiles` / `vaultCard.menu.remove` 已走 i18n，不額外處理

##### G-04a-2 · system chrome 議題（CTA 跟 Windows controls 之間留白）[shared] [defer]
- **問題**：截圖最右 Windows system controls (min/max/close) 跟 amber CTA 之間留 140px（`Lobby.tsx:91` 預留給 WindowControls）
- **設計稿假設**：`Tauri v2 desktop window, no browser chrome` — 完全自繪 / frameless
- **影響**：視覺斷層、CTA 不貼 viewport 邊
- **決策**：**擱置**——這牽動全 app（不只 04a），frameless 是大工程，且目前可用，留到設計 audit 結算後再單獨評估是否值得做

#### 驗收條件（不開 Gap，但結算時要驗）

- **path overflow**：`VaultCard.tsx:51` 已有 `truncate max-w-[60%]`，需用很長 path（例如 `D:/very/deep/nested/folder/structure/repo-name-very-long`）實測 ellipsis 行為對不對

---

## Cross-cutting · 字號 scale

> 確認 2026-05-25。harry 在 1920×1080 + Windows 100% 縮放下實機反饋「整體文字偏小、看起來空、有點吃力」。
>
> 設計稿 type scale 是 **Linear-tight**（13px body / 11px meta / 10px micro），合理推測設計師在 macOS retina 環境做 mock，沒考慮到 Windows 100% 縮放實際渲染密度。

### 決策（harry 確認 2026-05-25）

**全 token bump +1 級**：

| token | 設計稿原版 | **新規格** | 用在哪 |
|---|---|---|---|
| body | 13 | **14** | row text, nav items, 副標, Quickstart steps |
| body-lg | 14 | **15** | quiz choices |
| meta | 11 | **12** | timestamps, counts, paths |
| micro | 10 | **11** | section labels uppercase tracked |
| h-row | 18 | **20** | screen titles (Goals) |
| h-detail | 20 | **22** | goal title |
| h-quiz | 22 | **24** | quiz question |
| h-empty hero | 24 | **28** | empty-state hero |

### 理由

- 14px body / 12px meta 是 Windows desktop app 常規舒適範圍（VSCode default 14、JetBrains 13-14）
- 28px empty hero 在大留白下會撐得更有重量感，順帶減輕 G1 大留白感受
- +2 級保留作為「+1 級實機一週後仍偏小」的備案；先 +1 觀察

### 影響範圍 / 結算階段要做的事

- Tailwind config（`tailwind.config.ts`）或 `globals.css` 設計 token 改字號
- 全 component sweep 把 hard-code `text-[Npx]` 換成 token 或調整對應值
- snapshot test 重生（vitest visual snapshots）
- 在 1920×1080 100% 縮放 + 4K 150% 縮放兩個基準點都看一次

### 驗收

- +1 級 landed 後在 1920×1080 100% 縮放使用一週
- evaluate 是否升 +2 / 加 ODI-5（user-level font scale setting，詳見 Open Design Ideas 區）
- **design v1 reply 補充**：重新 snapshot 時也要 spot-check **4K @ 150% scaling + macOS retina baseline**；若 macOS 看起來「太肥」是要 ODI-5（user font scale）的訊號

---

## Cross-cutting · Motion Vocabulary（design v1 reply 確認 2026-05-26）

> codebus brand motion 鎖定 **2 個 mood**，新動畫提案必須 map 到其中一個、否則直接 reject。

### 2 個合法 mood

| Mood | 動作 | 用在 | 概念 |
|---|---|---|---|
| **Moving forward** | `codebus-bus-roll`（translate -26→12px + ±2° rotate + slight Y bob, 1.8s loop）| LoadingOverlay (vault init) | 「codebus 正在做有終點的工作」 |
| **Idling in place** | 垂直 2px bob + 水平 1px jitter, 1.4s loop, **無 rotation** | 04b Lobby empty hero | 「codebus 在等你」 |

### Hard Nos

| ❌ 禁止 | 原因 |
|---|---|
| Goal Running 加 bus 動畫 | 已有 amber pulsing dot + stream-tail caret——再加 bus motion 會競爭注意力。**pulsing dot 就是 「codebus 正在開車」 的 affordance**（don't compete） |
| Quiz generation 加 bus 動畫 | 同上理由；且會稀釋 LoadingOverlay 的 「moving forward」 mood 獨佔感 |
| Wordmark 🚌 動畫 | 靜態 glyph + 2 個 deliberate motion moment = 對的 cadence；3+ 隻動畫 bus 會 tip 到 mascot-overuse |

### 必加約束

- 所有 bus motion **必須 gate on `@media (prefers-reduced-motion: reduce)`**，fallback 完全靜態 🚌（無 transform）
- ODI-1 Bumpy road 就是 「Idling in place」 mood 的具體規格——locked，不另開
- ODI-4 ChatWidget collapsed amber pulse dot 是「Running ambient」 的官方 affordance、不是 bus motion 議題

---

## 額外畫面 · LoadingOverlay

> **不在 design-handoff README 6 個核心畫面內**——是 codebus 團隊自加的過場（addVault init-heavy 分支時的全屏 overlay）。沒對應 spec、所以 review 只盤點議題、不對照 gap。

- **現況截圖**：`c:\Users\harry\Downloads\螢幕擷取畫面 2026-05-25 170444.png`（harry 提供，2026-05-25）
- **現況實作**：
  - `codebus-app/src/components/LoadingOverlay.tsx`（72px 🚌 + `codebus-bus-roll` 1.8s 動畫 + 標題 + 副標）
  - `codebus-app/src/styles/globals.css:39`（`@keyframes codebus-bus-roll` — translateX -26→12px + translateY -3px + ±2deg rotation）
  - `codebus-app/src/i18n/messages.ts:66,300`（`loading.title` / `loading.subtitle`）
- **觸發點**：`App.tsx` `initInProgress` flag 來自 `useVaultsStore`，addVault init-heavy 分支期間 true

### 議題

#### LOI-1 · 動態 progress（live step display）[idea]

- **動機**：harry 希望「正在處理什麼」顯示在副標，取代靜態枚舉「複製 source、掃 PII、寫 wiki 結構、建巢狀 git」
- **可行性**：**backend 已經做好一半**
  - `codebus-core/src/vault/init.rs` `run_init` 已 emit 20+ 個 `InitEvent`（Start / LayoutCreated / SourceGitignore / PII warn / RawSyncDone / InternalGitignoreDone / NestedRepoDone / SchemaDone / ManifestDone / NavStubsDone / SkillBundlesDone / SettingsDone / ObsidianResult / CommitDone / StarterConfigDone / Finished 等）
  - 但 `codebus-app/src-tauri/src/ipc/vault_list.rs:207` 把 `on_event` closure 設成 noop `|_| {}`，**所有 event 丟到水溝裡**
- **要做的事（粗略）**：
  - Tauri 層：`add_vault_at` 改 async + accept AppHandle，把 InitEvent → Tauri event `vault-init-progress` emit 給 frontend
  - Frontend：LoadingOverlay listen 該 event、用 state 保存當前 step、render 動態副標
  - i18n：新增 `loading.step.<name>` zh + en key
  - IPC contract：新增 `vault-init-progress` event 規格
- **階段切分建議**（20+ InitEvent 太細，閃過太快 user 看不清，建議收斂成 6 階段）：
  - 1 Start + LayoutCreated + SourceGitignore → 「準備車庫…」
  - 2 PII config 載入 + RawSyncDone → 「複製源碼並掃描敏感資料…」
  - 3 InternalGitignoreDone + NestedRepoDone → 「設定獨立 git…」
  - 4 SchemaDone + ManifestDone + NavStubsDone + SkillBundlesDone + SettingsDone → 「建立 wiki 結構…」
  - 5 ObsidianResult → 「註冊到 Obsidian…」（skipped 時跳過）
  - 6 CommitDone + Finished → 「最後上路檢查…」
- **文案候選**：上表都是 brainstorm 初稿，**未確認**
- **範圍提醒**：這條超出 design audit 範圍——涉及 backend / Tauri IPC / 新增 event channel，**不只是換文案**。實作建議走獨立 spectra change（命名候選 `loading-overlay-live-progress`），有自己的 proposal / tasks / spec delta
- **狀態**：idea；何時做留到 design audit 結算階段一起排序

#### LO-1 · 副標把實作細節曝光 [open]

- **問題**：副標「建立 vault 中：複製 source、掃 PII、寫 wiki 結構、建巢狀 git」對沒做過 security / 不懂 git internals 的 user 是黑話
- **特別敏感**：「掃 PII」可能誤導 user 以為「你掃了我的什麼資料」；實際是掃**源碼**裡是否有像 API key 的東西要 redact
- **與 LOI-1 關係**：若 LOI-1 做了，這條自動消失（動態 progress 替代靜態枚舉）；若 LOI-1 不做，這條需獨立修文案
- **狀態**：open；視 LOI-1 決議

#### LO-2 · 標題「公車正在發車…」文案 [open]

- **harry 反饋**：對「公車正在發車…」措辭可能不滿（user 主動要求找這個文字）
- **候選方向**（未收斂）：「準備出發…」 / 「上車中…」 / 「公車進站中…」 / 「司機暖車中…」 / 維持
- **狀態**：open，待 harry 確認方向

#### LO-3 · 動畫詞彙表一致性 [shared] [open]

- **問題**：`codebus-bus-roll` 是「位移 + 旋轉 + 上下顛簸」**真在前進**動畫；ODI-1 是「Bumpy road **原地** 顛簸前進」
- **影響**：兩個 bus motion 用途不同但風格相關，應該有**動畫詞彙表**：行進中 = LoadingOverlay、待機怠速 = 04b hero、loading inline spinner = TBD（02 Goal Detail running 可能會有）
- **狀態**：open，等審到 02 後一起決定

#### LO-4 · 「3-15 秒」實測 [verify]

- **要驗**：副標承諾 3-15 秒，若實際常 ≥ 30s user 會以為 hang
- **狀態**：實作 LOI-1 或修文案前要先測量

---

## 01 · Vault Workspace

- **現況截圖**：`codebus-app/scripts/.workspace-current.png`（2026-05-25，cc-haha vault 空狀態）
- **規格來源**：
  - `design-handoff/README.md` § `01 · Vault Workspace`
  - `design-handoff/design_files/components/vault-workspace.jsx` + `sidebar.jsx`
- **現況實作**：
  - `codebus-app/src/components/workspace/Workspace.tsx`（sidebar + main pane）
  - `codebus-app/src/components/workspace/GoalsTab.tsx`（empty state）

### Sidebar Gap

##### S1 · 「← Back to Lobby」英文 hard-code [i18n Cat B]
- **問題**：`Workspace.tsx:250` 純英文 `← Back to Lobby`
- **決策**（harry 確認 2026-05-25）：翻成「← 返回」
- **理由**：「Lobby」沒在 UI 其他地方出現（Lobby 畫面本身用 🚌 codebus wordmark，不寫 "Lobby"），user 沒語境學到這個詞；用通用 back-navigation pattern 最直接
- **i18n key**：新增 `workspace.sidebar.back`；en 對應 `← Back` 或 `← Lobby`（en 環境可保留 Lobby，待 i18n 結算階段確認）
- **連動**：`Workspace.tsx:263` `title={...Click to open in file explorer}` 也要 i18n（Cat B 補洞）

##### S2 · vault name + path 區塊上下分隔線太淡 [shared G5]
- **問題**：套 `border-t border-border` 但實機看不到，跟 04 Lobby 的 G5 同對比度問題
- **修法**：跟 G5 共用結算階段方案（border-strong 或 token 整體校準）

##### S3 · Nav 缺 section label — **drop 整個 section label** [local]
- **設計稿**：nav 區頂部 uppercase 10px tracked「VAULT」label
- **決策**（harry 確認 2026-05-25）：**直接 drop 整個 section label**，不補
- **理由**：
  - 只有 3 個 tab、沒分組，section label 是視覺噪音
  - 「Vault」詞已從 UI 拿掉（G-copy-2），原本的「VAULT」label 也沒位置可放
  - 設計稿 section label pattern 適合多群組 nav（Linear 有 Views / Favorites / Teams），codebus 一群組不適用
- **跟 G4 解耦**：G4 是 04 Lobby「快速開始 / 近期」section label 視覺強化問題；S3 是直接不要 section label。互不影響

##### S4 · Nav rows 缺 emoji prefix [local]
- **問題**：現況純 text `Goals / Wiki / Quiz`
- **設計稿**：🚏 Goals / 📂 Wiki / 🎓 Quiz（README bus metaphor "medium dose" 明確規定）
- **修法**：`Workspace.tsx:270-298` TabButton 加 emoji prop / 直接寫進 label
- **i18n 連動**：tab labels 仍保留英文（D1 例外），但 emoji 是視覺元素不走 i18n

##### S5 · Nav rows 缺右側 mono count [local]
- **問題**：每個 tab 沒顯示 count
- **設計稿**：每個 row 右邊 mono 11px count（goals count / wiki page count / quiz count）；empty 時顯 0 是 informational
- **修法**：TabButton 加 count prop；資料來源是現有 goalRuns / wikiPages / quizzes store
- **size 跟字號 cross-cutting 連動**：mono 11 → 12（+1 級 bump）

##### S6 · Active row 缺左側 2px amber bar [local]
- **問題**：現況 active row 是 amber tint 整塊填充、無左 bar
- **設計稿**：active row `bg-active` + `border-strong` + 左側 2px amber bar (`left: -6px` overflow 出 row）
- **影響**：「你在哪一頁」視覺定位較弱；這個 left bar 是 Linear / Raycast 標誌性 active-state pattern
- **修法**：TabButton active 狀態加 pseudo-element / 絕對定位 2px amber bar

##### S7 · Sidebar 缺底部 footer [local] [partial]
- **設計稿**：sidebar 底部有 settings icon + refresh icon + `⌘K` kbd chip flush right
- **現況**：sidebar 底部空白；settings 在共用 BottomStrip
- **決策**（harry 確認 2026-05-25，updated 2026-05-26 為 05 CUT 後 ⌘K 仍綁 ChatWidget toggle）：
  - ✅ 加 **settings icon button** 到 sidebar 底部
  - ❌ **Drop refresh icon**——codebus 有 file watcher、不需手動 refresh；多一個按鈕是視覺噪音
  - ✅ **`⌘K` kbd chip 仍保留**——05 雖 CUT，但 Cmd+K 仍綁 ChatWidget toggle（見 `useChatShortcut`），chip 標示 keyboard shortcut 仍有意義
- **連動 F1**：Workspace 隱藏 BottomStrip（settings 搬到 sidebar 後 BottomStrip 失去存在意義，只剩 v3.0.0 版本號）
- **F1 修法選項**：
  - a. BottomStrip 改 Lobby-only（App.tsx route check 後再 render）
  - b. BottomStrip 整個移除，Lobby 也用其他方式顯示 version（例如 lobby footer）
  - 結算階段定

### Right Pane / Empty State Gap

> empty state 方向：**Y · 借鏡 04b Lobby empty 的 hero + Quickstart-like card 風格，但用 Goals 自己的語彙**（harry 確認 2026-05-25）

##### R1 · 28px topbar 缺 1px bottom border [shared G5]
- 跟 G5 / S2 共用對比度問題；結算階段一併處理

##### R2 · Empty state 缺 content header row [local]
- **設計稿**：頂部 `<h1>Goals</h1>` (20px h-row，字號 cross-cutting +1 級後 → 20px) + 副標 + `[+ New goal]` CTA + `N` shortcut chip
- **現況**：直接跳 empty state 中央三行，沒 header row
- **決策**：保留 header row（即使 empty 也有 title 給「你在哪頁」定位、CTA 隨時可達）
- **i18n**：副標跟 CTA label 新增 i18n key

##### R3 · Empty state 視覺設計（design 灰區，走 Y 方向）[local]
- **設計稿沒覆蓋 Goals empty state**——現況「中央三行說明 + 三個範例 quote」是團隊自製
- **新版三段式 layout（待結算階段定稿，brainstorm 版本）**：
  1. 上方 header row：`Goals` h1 + 副標（候選：「列出你想搞懂的事，公車一站一站讀給你看。」）+ `[+ New goal]` CTA
  2. 中央 hero：🎯 emoji (40-56px) + 「還沒有任務」h-empty + 一句引導
  3. 下方 examples card：3 個範例做成 **amber-tinted mono pills**，**可點擊直接 prefill NewGoalModal**（提升 discoverability）
- **保留現況 3 個範例**（authentication flow / data ingestion / public API）但翻成 zh + 改成 pill 互動
- **連動 R5**：三段式 layout 撐起垂直空間，G1 大留白自動緩解

##### R4 · Empty state 英文 hard-code [i18n Cat B]
- `GoalsTab.tsx:18-20` 三個 examples
- `GoalsTab.tsx:86` `+ New goal`
- `GoalsTab.tsx:95` `Click + New goal to ask codebus to ingest something into the wiki`
- 全部走 R3 新文案 + i18n bundle

##### R5 · Content 垂直幾何置中、大留白 [shared G1]
- 走 R3 三段式 layout 後自動緩解；無需單獨修

##### R6 · `+ New goal` CTA 位置 [local]
- **現況**：右上 topbar 區（跟 sidebar back link 平行的 row）
- **設計稿**：在 content header row 內（右側）
- **決策**：跟 R2 header row 一起做，CTA 搬進 header row
- **觸發**：保留現有 `N` keyboard shortcut

##### R7 · 右下浮動 ChatWidget [local] [open]
- **現況**：圓形 floating button 右下，點開 ChatWidget
- **設計稿**：沒提這個（design 6 個畫面 chat-like 只有 cmdk-overlay）
- **決策**：保留 ChatWidget 存在（codebus 自加功能、跟 design 平行）；但 **UIUX 行為待另外討論**（harry 提出 2026-05-25）
- **i18n Cat C 補洞**：`aria-label="Open chat"` / `"Resize chat widget"` / `"Minimize chat"`、`title="Drag to resize"`
- **詳細討論結果見**：下方 R7-Discussion 區

##### R7-Discussion · ChatWidget UIUX 行為（confirmed 2026-05-25）

###### R7-1 · 💬 emoji [resolved - keep, design v1 pushback ack]
- **原方案**：換成 lucide `MessageSquare`
- **Design v1 reply pushback**：codebus 是 「friendlier-than-Linear sibling」、warmth 全靠 deliberate emoji moments（🚌 wordmark / LoadingOverlay / 旅行日誌）；如果 💬 是 chat 的那一個、coherent
- **決策**（harry 確認 2026-05-26）：**保留 💬**——不換 MessageSquare、不換 🚌
- **理由**：「The only cartoon element left」是 feature 不是 bug；codebus 不是 Linear、是 warmer sibling
- **不做的事**：不加 active goal pulse dot 到圓鈕本身的 emoji；pulse dot 走 ODI-4 規格（右上小 dot、emoji 不變）
- **狀態**：closed

###### R7-2 · 位置衝突：圓鈕擋 RunDetailRunning Cancel button [bug] [shared with 02]
- **問題**：goal running + ChatWidget collapsed 時，`ChatWidget.tsx:127-128` 圓鈕 `fixed bottom-right` 堆疊 `RunDetailRunning.tsx:101` Cancel button footer `justify-end`
- **修法**：**不動 ChatWidget 位置**，改 Cancel 位置——把 Cancel 從 footer 搬到 02 spec 規定的 header right（跟 elapsed / tokens 同 row）
- **設計稿依據**：02 Goal Running spec 明寫 Cancel 在 right side of status line（"red-tinted border, never red fill"），現況 implementation 本來就跟 spec 不一致
- **連動**：標 `[shared with 02]`——等審 02 時把這個修法跟 02 其他 Cancel button 規格（red-tinted border）一起做
- **順帶 i18n**：`"⏹ Cancel"` / `"Cancelling…"` hard-code 英文（`RunDetailRunning.tsx:108`）→ 補進 Cat B
- **跟 ODI-4 的共構視覺契約**：Cancel 搬 header 後，bottom-right ChatWidget 圓鈕成為唯一右下元素；ODI-4（active goal pulse dot）在這位置加 amber pulse 提示 goal 進行中。**契約**：未來右下位置不再新增其他 action button，避免再撞位（任何「跟 goal status 有關」的視覺都收進 ChatWidget 圓鈕 ODI-4 系統）

###### R7-3 · ChatWidget 跟 05 Cmd+K Overlay 關係 — 砍 05
- 決議見下方 `## 05 · Cmd+K Overlay (CUT)` 區
- ChatWidget 保留 Cmd+K toggle（現況不變）
- 未來若要 spotlight 體驗：ChatWidget 加「centered modal mode」見 ODI-3

### Goals Populated Gap

- **現況截圖**：`codebus-app/scripts/.goals-list.png`（2026-05-25，3 個 goal row：1 done + 2 interrupted）

##### GP1 · 缺 content header row [shared with R2]
- **設計稿**：`<h1>Goals</h1>` (h-row 20px) + subtitle + `[+ New goal]` CTA + `N` shortcut chip 同 row
- **現況**：直接跳到 goal row 列表，沒 header
- **修法**：跟 R2 共用（empty/populated 共用 header row component）

##### GP2 · `+ New goal` 位置錯 [shared with R6]
- 現況在右上 topbar；應在 content header row 右側
- 跟 R6 一起修

##### GP3 · 缺 `RECENT` section label + count [shared with G4 / S5]
- 設計稿：「RECENT」micro-label + `3 of 3` mono count
- 現況：完全沒這個 row
- 修法：跟 G4 中文 section label 替代方案 + S5 nav count 樣式一起做

##### GP4 · Goal table 缺 card wrapper [local]
- 設計稿：rows 在 `bg-raised` card with `border`
- 現況：rows 直接攤在 viewport，沒邊框、沒底色
- 修法：用 `bg-raised` card 包覆整個 goal table，邊框跟 G5 共用 token 校準

##### GP5 · Status indicator 三態系統 [local] [updated 2026-05-25 from 02 Interrupted view]
- **狀態語意（confirmed via 02 Interrupted view）**：
  - **Done**：`green 7px dot`（goal 完整跑完）
  - **Interrupted**：`amber ⚠️` 或 `amber 7px dot`（app 被關 / user cancel / 中途死掉，**可 retry**）
  - **Failed**：`red 7px dot`（LLM/系統錯誤、**不可 retry**）
- **現況**：done = white ✓ check；interrupted = amber ⚠️；failed 沒實例觀察
- **設計稿**：只區分 done / failed 兩態；codebus 三態系統更完整、**保留**
- **修法**：
  - 統一 7px dot 系統（去掉 ✓ check icon）
  - done green / interrupted amber / failed red
  - row hover 時 dot 旁顯示文字 tooltip 增強可讀性（optional）

##### GP6 · 缺 kebab + hover affordance [shared with G-04a-1]
- 設計稿：row 右側 kebab `⋮`，hover 顯示（opacity 0→1）
- 現況：完全沒 kebab、無互動 entry point
- **shared with G-04a-1 的範圍 = 同 affordance pattern（hover-revealed kebab + 右鍵 fallback）**，但 **menu items 各自設計**：
  - vault card menu：Reveal in files / Remove from list
  - goal row menu：Retry / Cancel (running 時) / Open in new tab / Delete attempt
  - quiz history row menu（QL5）：Retry / Delete attempt / Export answers
- 實作建議：抽 `<HoverKebabMenu items={...} />` 共用元件；各 row 自己決定 menu items

##### GP7 · time-ago 沒走 i18n [i18n Cat B]
- 現況：`34m ago` / `2h ago` 英文 hard-code
- `i18n/messages.ts:25` 既有 `common.minutesAgo` / `common.hoursAgo` / `common.daysAgo` key
- 修法：呼叫 `t("common.minutesAgo", { n: 34 })` → zh 「34 分鐘前」/ en `34m ago`
- 補進 i18n Cat B sweep

##### GP8 · Running row 設計沒落地 [local] [HIGH PRIORITY, design v1 強烈表態]
- **設計稿**：running row expanded 顯示 mono 11px stream tail（"current action narration"）+ 1px amber blinking caret + 右側 `streaming · 4,218 tok` amber
- **現況**：running 時 row 跟其他 row 同樣 collapsed，user 不點進 detail 看不到進度
- **影響**：ambient awareness 不足；user 不知道「現在在做什麼」、會反覆切回去看
- **Design v1 表態（2026-05-26）**：「**這是 v1-spec 最 attached 的 affordance、audit 正確 flag**；少了它 running goal 跟 done goal 視覺一樣、user 會頻繁 check back。**Don't deprioritize**.」
- **修法**：running row 自動 expand、顯示最新 ActivityStreamItem 內容 + blinking amber caret + 右側 token count
- **連動 ODI-4**：ChatWidget collapsed pulse dot 是同樣「ambient awareness」需求；amber pulse = codebus running 的官方 ambient sign
- **Priority**：升 high、跟 critical bug 一起做（不是 polish）

##### GP9 · 重複 goal 沒視覺區分 [design 灰區] [skip]
- 截圖中兩個一樣的 goal title，spec 沒覆蓋 retry / duplicate 行為
- **決策**：暫不開 gap；後續若 user 抱怨再評估

---

## 02 · Goal Detail

- **現況截圖**：
  - `codebus-app/scripts/.goal-running-1.png`（2026-05-25, 02a running 早期）
  - `codebus-app/scripts/.goal-done.png`（2026-05-25, 02b done）
- **規格來源**：
  - `design-handoff/README.md` § `02a / 02b · Goal Detail`
  - `design-handoff/design_files/components/goal-detail.jsx`
- **現況實作**：
  - `codebus-app/src/components/workspace/RunDetailRunning.tsx`
  - `codebus-app/src/components/workspace/RunDetailDone.tsx`
  - `codebus-app/src/components/workspace/RunDetailCancelled.tsx`
  - `codebus-app/src/components/workspace/ActivityStreamItem.tsx`

### 02 哲學決策 · Timeline structure（philosophy）

> 設計稿 vs 現況有 **結構性差異**，需先定方向再說細節：
>
> - **設計稿**：「**timeline + collapsible card**」結構——READING CODEBASE / WRITING WIKI 兩個 collapsible section、各自有 1px-left-border guide + tick marks、stream log card 在底部 collapsible 預設 closed
> - **現況**：「**flat activity feed + emoji banner**」結構——所有 event 混一條 feed、有 🚌🎯🤔🔧 banner + thought block + tool rows

**決策（harry 確認 2026-05-25，採我建議的混合方案）**：

1. **保留現況 banner emoji 風格**——🚌 來囉來囉 / 🎯 任務目標 / 🤔 思考 / 🔧 工具 等是 codebus brand 特色，不丟
2. **加 visual grouping**——tool 用 visual grouping (Read/Glob/Grep 一群 / Write/Edit 一群 / Shell 一群)，配 1px left-border guide + tick marks（向設計稿靠攏）
3. **stream log 拆出**——把 raw event log（含 timestamps、color-coded tags）拆到下方獨立 collapsible card，**預設 closed**，跟 spec 對齊

→ 主視圖留 brand + visual grouping，細節留給 collapsible stream log。

### 02a · Running Gap

##### W2 · Back link 排版錯 [local]
- **設計稿**：`← Goals` back link 在 title **上方獨立一行**
- **現況**：`← back` 跟 title `完整研究專案目的` 同一 row 並排
- **修法**：back link 拉到 title 上方獨立 row，font-size 12 / fg-tertiary
- **連動 i18n**：`← back` 英文 hard-code → 翻成「← Goals」（en）/「← 返回 Goals」（zh）；保留 Goals 當 tab jargon

##### W3 · Status line 排版錯（解 R7-2 collision）[local] [shared with R7-2]
- **設計稿**：title 同一 row 右側：pulsing amber dot + amber `Running` + mono `23s · 8.2k tokens` + danger-toned `Cancel` button（red-tinted border, never red fill）
- **現況**：dot+Running 在 topbar 右；elapsed/tokens **換行左對齊**；Cancel **在 footer 右下**（跟 ChatWidget 圓鈕堆疊）
- **修法**：所有元素搬進 title row 右側、依 spec 排列；footer 整個拿掉
- **連動 R7-2**：Cancel 搬 header 後跟 ChatWidget 圓鈕不再衝突

##### W4 · Activity stream 2-phase clustering [local] [design v1.5 spec lock]
- **設計稿**：READING CODEBASE / WRITING WIKI 兩個 phase cluster + 1px-left-border guide + 8px tick marks
- **決策**（walkthrough-decisions.html §02 lock 2026-05-26）：**2-phase semantic split**（取代我原 3-5 tool kind grouping）
  - **READING CODEBASE cluster**（intake phase）：Read / Glob / Grep + Shell kind=`read` + Shell kind=`inspect`
  - **WRITING WIKI cluster**（output phase）：Write / Edit + Shell kind=`mutation`
- **Tool kind → cluster mapping table**（walkthrough-decisions §2.2 lock）：

| tool | cluster | icon prefix (mono ASCII) |
|---|---|---|
| `Read` | READING CODEBASE | `📄` |
| `Glob` | READING CODEBASE | `🗂` |
| `Grep` | READING CODEBASE | `🔍` |
| `Shell` kind=read | READING CODEBASE | `$_` |
| `Shell` kind=inspect | READING CODEBASE | `$?` |
| `Write` | WRITING WIKI | `✎` |
| `Edit` | WRITING WIKI | `✎` |
| `Shell` kind=mutation | WRITING WIKI | `$!` |

- **5 條細節規範**（walkthrough-decisions §2.4 lock）：
  1. **Brand emoji banner 不收進 cluster**——🚌 / 🎯 / 🤔 是敘事層、跨 cluster 流動、flat 排在 cluster 之間當段落轉折
  2. **Cluster 可重複出現**（thought→reading→thought→reading→thought→writing）——cluster 是「同類連續發生」的視覺收斂，不是 phase 的單例
  3. **Live tail（W5）只在當前正跑的 cluster 最底**——不是每個 cluster 都顯示
  4. **Cluster collapsible**：default **open during run / closed when complete**
  5. **Cluster count 不算 thought blocks**——「12 calls」只算真有 tool call 的 row
- **02b Done cluster collapsed** 顯示 summary（walkthrough-decisions §2.5）：`Reading codebase · 12 reads · 195 shell · 6.2s`、`Writing wiki · 3 new · 2 updated · 4.5s`（**對齊點 5 決議：中文化** → 「讀檔 12 次 · shell 195 次 · 6.2 秒」 / 「新增 3 · 更新 2 · 4.5 秒」）
- **保留**：現有 banner emoji + thought block 顯示風格不變
- **Icon 用 mono ASCII 不用 emoji**——這層是技術 ambient、不該再加 brand 量；brand emoji 留給 phase banner（🚌 / 🎯 / 🤔）跟 commit / done banner（🚏 / 🎉）

##### W5 · Stream tail（live narration）缺失 [local]
- **設計稿**：bottom row 顯示 spinning circle + amber narration text（"analyzing token validation flow…"）+ 1px amber blinking caret
- **現況**：最後一個 event 跟其他 row 同等顯示，沒突出
- **修法**：當 goal still running 時，最末 row 加 spinning indicator + amber 文字 + blinking caret

##### W6 · Stream log card 完全缺失 [local]
- **設計稿**：bottom collapsible card，內含 raw stream log——11.5px mono、`00:00.142` timestamps、color-coded `goal` / `plan` / `read` / `write` tags、**closed by default**
- **現況**：完全沒這個 card，所有 raw events 直接 inline 在 activity feed
- **修法**：把 raw event log 抽到底部 collapsible card，預設 closed；feed 留 banner + summarized tool calls

##### W7 · Shell tool row 可讀性差 [→ X1]
- 由 X1 cover；不獨立修

##### W10 · "← back" 等英文 hard-code [i18n Cat B]
- `RunDetailRunning.tsx` back button label「← back」
- 補 i18n bundle

### 02b · Done Gap

##### D1 · Section label 中英混雜 [shared G4]
- **問題**：`COVERED PAGES` / `LINT` 維持英文 uppercase；中文版環境裡跟「注意事項」配 uppercase 風格不一致
- **修法**：跟 G4（04 Lobby 中文 section label 視覺強化）共用替代方案
- **連動**：D4 命名統一一起做

##### D2 · 「注意事項」section 是 design 灰區 [local]
- **背景**：codebus 自加的「LLM 跑完留 follow-up notes」 feedback loop，設計稿 02b spec 沒覆蓋
- **決策（採我建議）**：**保留**——這是 codebus valuable feature；但向 design 靠攏：用同樣的 micro-label section header 風格（uppercase 或 G4 替代）
- **i18n**：「注意事項」label 走 i18n（en: `Follow-ups` 或 `Notes`）；內容 LLM 生成、不翻
- **驗收條件**（結算階段必驗、design v1 加嚴 2026-05-26）：
  - 跑 5 個不同類型 goal（研究 / 文檔生成 / lint fix / 跨模組分析 / quiz prep）看 follow-up notes 是否每次都產出
  - 品質可讀性檢查：notes 是否具體可 actionable、有沒有空白 / 全英文 / 太短不知所云的情況
  - **空白處理（design v1 嚴格）**：若**任一**goal 類型回 0 條 notes，section **必須完全 hide**（**禁止 render「Follow-ups: (empty)」**）；empty section 訓練 user skip → 殺死真有 notes 時的價值
  - 若 5/5 都有可讀 notes → 保留並推 design polish；若 <3/5 → 重新評估是否值得保留

##### D3 · TIMELINE card 預設展開（要驗）[local]
- **設計稿**：stream log card **closed by default**
- **現況**：截圖看到展開狀態——**待驗證**是預設展開還是 store 持久化了上次展開狀態
- **修法**：實機驗；若預設展開即違反 spec，改 closed by default
- **連動 W6**：02a 跑中的 stream log card 也用同一 default-closed 規則

##### D4 · `COVERED PAGES` vs spec `WIKI PAGES CHANGED` [local]
- **設計稿**：section heading `WIKI PAGES CHANGED`（強調「這次 goal 動了什麼」）
- **現況**：`COVERED PAGES`（強調「這次涵蓋了什麼」）
- **語意差**：spec 強動作；現況強範圍。動作版更直接、user 更容易理解
- **決策**：**改用 spec 「WIKI PAGES CHANGED」語意**；中文翻成「Wiki 頁面變更」或「這次動了哪些 wiki」
- **連動 D1**：跟其他 section label 一起走中英取捨

##### D5 · Codex shell wrapper [→ X1]
- 由 X1 cover；不獨立修

### 02c · Interrupted（**升 spec、不再灰區**，design v1 ack 2026-05-26）

- **現況截圖**：`codebus-app/scripts/.goal-failed.png`（2026-05-25）
- **實作**：`codebus-app/src/components/workspace/RunDetailCancelled.tsx` → **rename `RunDetailInterrupted.tsx`**（design v1 建議：state-vs-trigger 命名 drift 是 slow bug source）
- **背景**：設計稿 02 v1 只覆蓋 Running (02a) / Done (02b) 兩態；codebus 多了 Interrupted（user cancel / app close / network drop）。design v1 reply 確認**進 v1.1 spec 為 canonical 三態**
- **Spec routing rule**（design v1 確認）：
  - user-cancel / app-close / network-drop → **interrupted**
  - LLM API hard error / sandbox crash / lint loop irrecoverable → **failed**
  - 分不清楚？**default to interrupted**（retry 比 investigation 便宜）

##### I1 · Notice banner 文案 [i18n Cat B] [design v1 文案 ack]
- **現況**：「App was closed before this goal finished. Wiki state may be partial — review in terminal if needed.」（純英文 + terminal jargon）
- **新文案（design v1 reply 2026-05-26）**：
  - zh：**「中途離開了 codebus，wiki 還沒寫完。可以重跑這個 goal 補上。」**
  - en：「You left codebus before this goal finished. Wiki may be incomplete — retry to redo from scratch.」（待最終 wording）
- **替代我原版的理由**：原版「app 中途被關閉」 sounds blameful（是我做錯？codebus crash？）；新版「中途離開了 codebus」中性、「可以重跑」 比「需要的話」 更主動

##### I2 · `PARTIAL TIMELINE` 內容過簡 [shared with W6 / D3] [local]
- **現況**：只給 `reading 0 · writing 0 · other 8` 數字 summary，沒詳細 events
- **影響**：user 無法判斷 goal 中斷前做了什麼、是否該 retry
- **修法**：補 collapsible「展開完整 TIMELINE」（同 02b 的 stream log card pattern），預設 closed
- **抽 shared component**：W6（02a stream log）+ D3（02b stream log）+ I2/I5（02c partial timeline）= **三個 detail state 共用 `<CollapsibleStreamLog>` 元件**

##### I3 · `PARTIAL TIMELINE` label 中文 [shared G4]
- 跟其他 section label 一起處理中文 micro-label 視覺替代

##### I4 · `Retry with same goal` button 位置 + i18n [local]
- **現況**：在 main pane 右側中央孤立 button
- **設計稿**：其他 action（02b Done pill, 02a Cancel）都在 header right
- **修法**：搬到 header right（跟 status pill 同邏輯位置）
- **i18n**：英文 `Retry` 或 `Retry goal`；中文「重跑這個 goal」/「再跑一次」

##### I5 · 缺 timeline expansion entry [→ I2]
- 由 I2 cover（補 `<CollapsibleStreamLog>` 共用元件）；不獨立修

### 02 共用議題

##### X1 · Codex shell wrapper extraction + Shell kind 3-level enum [shared W7 + D5] [bug] [design v1.5 backend contract]
- **問題（原 UI 端）**：`ActivityStreamItem.tsx:187-191` `summarizeToolInput` 對 `obj.command` 直接 80 字截斷；Codex wrapper `"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe" -Command "<actual cmd>"` 吃掉 60+ 字、actual cmd 被截
- **修法 1**（UI）：`extractInnerCommand` helper detect powershell.exe / sh -c / bash -c wrapper、抽出 inner command 後再截 80 字
- **修法 2**（backend contract，design v1.5 lock 2026-05-26）：**Shell tool event 加 `kind` 欄位**
  - 三層 enum：`"read"` / `"inspect"` / `"mutation"`
  - 由 **skill 自己標**（codebus-goal / codebus-quiz / 等），不是 codebus-core 統一 default（**對齊點 3 決議**）
  - 對齊點 4 決議：不在表內的 tool（Task / WebFetch / 未來新 tool）擴 kind 加 `"other-read"` / `"other-write"`、走 READING / WRITING 對應 cluster；不開新 `cluster` 欄位（少一層）
  - 三層語意 + 範例：

| kind | 語意 | 範例 |
|---|---|---|
| `"read"` | 讀檔案內容 / 列目錄 / pattern match | `cat` / `head` / `ls` / `find` / `rg` / `grep` / `wc` |
| `"inspect"` | 讀環境 / 中性 introspection · 沒檔案內容 | `git status` / `git log` / `git diff` / `ps` / `env` / `which` / `npm ls` |
| `"mutation"` | 改 state（檔案 / git / 系統 / 安裝） | `rm` / `mv` / `mkdir` / `git commit` / `npm install` / `>file` redirect |
| `"other-read"` | 上表外、read-like 行為 default | （未來新 tool） |
| `"other-write"` | 上表外、write-like 行為 | （未來新 tool） |

- **拿不到 intent 時 default `"inspect"`**（最安全 unknown——不歸到 mutation cluster，避免「咦它寫東西了？」誤覺）
- **影響範圍**：所有 provider 的 Shell/Bash tool row 顯示變乾淨 + W4 cluster 歸屬有明確 signal
- **要驗**：Claude provider 的 `obj.command` 格式（可能本來就乾淨、無 wrapper）
- **示意（已在 chat 提過）**：
  ```ts
  function extractInnerCommand(raw: string): string {
    const ps = raw.match(/powershell(?:\.exe)?["']?\s+-Command\s+(.+)/i)
    if (ps) return ps[1].replace(/^["']|["']$/g, "")
    const sh = raw.match(/^(?:\/[\w/.]+\/)?(?:bash|sh)\s+-c\s+(.+)/i)
    if (sh) return sh[1].replace(/^["']|["']$/g, "")
    return raw
  }
  ```
- **要驗**：Claude provider 的 `obj.command` 格式（可能本來就乾淨，無 wrapper）

##### X2 · `<CollapsibleStreamLog>` shared component [shared W6 + D3 + I2/I5] [design v1 update]
- **背景**：02a/02b/02c 三個 detail state 都需要 collapsible stream log（02a 跑中可選展開、02b/02c 跑完看 raw events）
- **設計稿**：spec 寫「stream log card closed by default」、11.5px mono、color-coded `goal`/`plan`/`read`/`write` tags、`00:00.142` timestamps
- **修法**：抽 `<CollapsibleStreamLog events={...} defaultOpen={false} />` 共用元件，跨三 state 使用
- **Design v1 reply 補充（2026-05-26）**：**default-closed 要 per-user sticky via localStorage**——若 user 在 goal #1 expand 過，goal #2 自動 expand（記住「show me raw logs」 type 的 user）
  - localStorage key 建議：`codebus.stream-log.default-open` (boolean)
  - **不放進 Settings UI**——speculative；行為 sticky 就好

##### X3 · Header right action 位置統一 [shared W3 + I4 + 02b Done pill]
- **問題**：現況三 state header right action 位置不一致（02a Cancel 在 footer、02b Done pill 在 header、02c Retry 在 right-mid）
- **統一規格**：所有 state header right 都用 spec layout：title row 右側 = status pill / indicator + action button
  - 02a: amber dot + `Running` + elapsed/tokens + danger Cancel button
  - 02b: green `✓ Done` pill + elapsed/tokens
  - 02c: amber ⚠️ + `Interrupted` + Retry button

##### X4 · 三態 status 配色一致性 [shared GP5 + I1 + 02 status pills] [design v1 ack]
- **三態 token 系統**（design v1 確認進 spec 2026-05-26）：
  - done / pass → `--success #4ade80`
  - interrupted → `--warn #f5a623`（同 accent，故意 — codebus 三態保留設計稿 amber 主色）
  - failed / fail → `--error #f87171`
- **Running 狀態用同 amber 但加 motion + caret**——design v1 明說：「Running uses the same amber as interrupted but with an outer pulse ring + caret。**Don't try to find a fourth hue**.」
- **跨地方一致**：Goals list dot / Goal Detail header pill / Quiz history result tag / Wiki 變更 badge / Quiz completion summary hero（QF1 fail=red、pass=green） 全用同一組 token

---

## Wiki Tab（design 灰區，codebus 自加實作）

> 設計稿 README 明寫「wiki page reader is a future screen, not yet shown」——所以 Wiki tab **沒有 design spec 對照**，現況是 codebus 自加實作。
>
> 屬於 01 Vault Workspace 的一個 tab，但內容量大、議題獨立，所以單獨開節。

- **現況截圖**：
  - `codebus-app/scripts/.wiki-empty.png`（2026-05-26, vault 有 page 但未選 page 的狀態）
  - `codebus-app/scripts/.wiki-page.png`（2026-05-26, 選了「桌面工作台」module page）
  - `codebus-app/scripts/.wiki-page-bottom.png`（2026-05-26, 同 page 滾到底）
- **規格來源**：無 design spec；對照 codebus README + 5-bucket 概念（Karpathy LLM Wiki pattern）
- **現況實作**：
  - `codebus-app/src/components/workspace/WikiTab.tsx`
  - `codebus-app/src/components/workspace/WikiTree.tsx`
  - `codebus-app/src/components/workspace/WikiPreview.tsx`

### Wiki tree view Gap（vault 有 page 時左邊 tree）

##### WK1 · Section labels [shared G4] [design v1.5 lock]
- MODULES / PROCESSES / SYNTHESIS / CONCEPTS / ENTITIES 是 Karpathy 5-bucket taxonomy 的 domain term
- **對齊點 1 決議（2026-05-26）**：**保留英文 caps**，但走 G4 token 的 `.section-label--caps` 變體（11px / 0.08em letter-spacing / fg-tertiary、配 amber 2px bar）
- 理由：跟 README spec / CLI / Karpathy taxonomy 對齊；同 `Goals / Wiki / Quiz` tab 同等級 jargon discipline
- caps + tracking 從原 10px / 0.12em **收斂為 11px / 0.08em**——bar 來扛 ceremony、文字輕一點
- 跟 WK2 `OTHER` bucket 解散一致（OTHER 不是 spec taxonomy）

##### WK2 · 「OTHER」 bucket 太籠統 [local]
- 包含 Wiki Index + Goal Log（系統 pages、自動生成）
- 「OTHER」 一望無感
- **修法（採我建議）**：把 Wiki Index 移到 tree 頂部當主入口、Goal Log 移到 tree 底部 footer 區、**完全拿掉 「OTHER」 bucket**
- 5 buckets section 維持 concepts / entities / modules / processes / synthesis 純淨

##### WK4 · 空 bucket 不顯示（confirm by design）[verify]
- README 5 buckets，cc-haha vault 只見 modules / processes / synthesis（concepts / entities 沒 page）
- **驗證結果**：根據 `WikiTab.tsx` 與 `WikiTree.tsx` 行為推測是 by design——只 render 有 page 的 bucket，empty bucket 隱藏
- **狀態**：暫標 by design；結算階段確認後決定

##### WK5 · Tree row 缺 visual cue [local] [design v1 ack]
- 每 page 純文字 row、沒 icon、無法一眼分辨同 bucket 內各 page 類別
- **決策**（design v1 reply 2026-05-26）：lucide / **單色 `fg-tertiary` / 14px / 不上色**——never colored avoids competing with status dots
- **Lucide mapping table**（design v1 spec）：
  - `Lightbulb` · concept
  - `Box` · entity
  - `Blocks` · module
  - `Repeat` · process
  - `Link` · synthesis
- **若 usability 測試發現太 noisy**：fallback 砍成 2 glyph（concept/synthesis = 抽象 / entity/module/process = 具體），但先做完整 5 個

##### WK6 · column 之間缺 visual separator [shared G5]
- sidebar (200px) | wiki tree (~240px) | preview pane 之間 border 看不到
- 跟 G5 border 對比度共用修法

##### WK7 · 「Wiki Index」 / 「Goal Log」 命名 [design v1 ack]
- system pages、自動生成；命名 hard-code 英文
- **決策**（design v1 reply 確認 2026-05-26）：
  - `Wiki Index` → 「Wiki 索引」（中文，跟其他 page 中文 title 風格一致）
  - `Goal Log` → **「旅行日誌」**（呼應 README brand `log.md` 旅行日誌）；design v1 評語：「考慮過直譯『公車日誌』但太 literal、奪 user 注意力；『旅行日誌』 sits 一層抽象——是 user 在 codebase 的 trip、由 codebus 記錄；matches the file 比工程 framing 好」
  - **EN counterpart**：keep `Goal Log` 為 canonical 英文名（match CLI / file path）
- **Bonus（design v1 加）**：旅行日誌 page 頂部加一次性 framing subtitle 「your trip through this repo」（en） / 「你在這個 repo 的旅行記錄」（zh）
- 連動 WK2（拿掉 OTHER bucket 後，這兩個 page 重新定位）

##### WK8 · 🗂 icon 功能 = wiki tree toggle [observation]
- `WikiTab.tsx:88` 用 lucide `<Folder>`，是折疊/展開 wiki tree 的 button
- `aria-label="Toggle Pages tree"` hard-code → **i18n Cat C 補洞**（已記）
- **狀態**：功能明確，視覺 OK，不開 gap

##### WK9 · 缺 search / filter [future, no gap]
- 小 vault 沒問題；大 vault page 多時難找
- 不開 gap、未來再說

### Wiki page preview Gap（選了 page 後右邊渲染）

#### 實測 confirmed 觀察（不是 gap）

- **WP3 · 底部「Quiz me on this」 button** — 確認存在 ✅，跟「在 Obsidian 開啟」並排（待 WP10 樣式 polish）
- **WP4 · Wikilinks 渲染** — 確認用 `[文字](codebus://wiki/.../slug)` 形式渲染為 normal link（樣式議題見 WP11）

##### WP1 · Preview pane 寬度比例 [verify]
- sidebar (200) + tree (~240) + preview (~750~1480) → 1920×1080 縮放下實際寬度待測
- CDP 截圖會等比縮，可能不準
- **狀態**：實機驗；若 < 1000px 考慮 tree column 預設折疊或縮窄

##### WP2 · Page metadata bar [local] [design v1 spec ack]
- **位置**：直接放 page title 下方、body content 之前
- **規格**（design v1 spec 2026-05-26）：**single line, mono-12（post-bump）/ fg-tertiary**
- **內容**：**provenance only**——goal name + time + source count，**禁止加**：word count / tag chips / view count / 任何 analytics flavor
- **格式範例**：`Last updated by [完整研究專案目的] · 12h ago · 4 sources`
- **互動**：
  - goal name **hovers + clicks → 跳該 goal 的 detail view**（這是 codebus 最重要的關係 closure：「this page exists because this goal ran」）
  - `4 sources` = page body 的 wikilink-citation count，**only show if > 0**
  - 若 page 被後續 goal 修改、列**最新** updater；不列 authorship history（那是 旅行日誌的工作）

##### WP5 · 缺 edit / regenerate action [design v1 ack]
- 看不到 edit page button
- **設計判斷確認 by design**——codebus 哲學是「不直接編輯 wiki，起新 goal 改」（人寫 vs LLM 寫的權責分隔）
- **引導 hint 位置**（design v1 確認）：放 **page footer**（不是 metadata bar）
- **文案**：`fg-tertiary` link「想改這頁？跑一個 goal 跟 codebus 說該怎麼改 →」 click 開 NewGoalModal pre-filled 「修改 [此頁] 的 ...」
- **理由**：metadata bar 是 provenance、不該 mix instructions；footer 是「下一步」的自然位置

##### WP6 · Code block 樣式 [verify]
- 深色背景 + mono font 大致 OK
- syntax highlighting library? token 顏色？需細查 `WikiPreview.tsx` 跟 Milkdown 設定

##### WP7 · Column 之間缺 1px border [shared G5]
- sidebar | tree | preview 三 column 間看不到分隔線
- 跟 G5 共用修法

##### WP10 · 底部 action button 樣式 + i18n [local]
- 「Quiz me on this」純英文 → i18n Cat B 補洞
- **翻譯**：「Quiz 這頁」（Quiz 保留 jargon、其他翻譯）
- 「在 Obsidian 開啟」中文 ✅
- **設計稿 02b 寫 Quiz me 該 amber tint**——現況兩 button 都 generic secondary；**修法**：Quiz me 改 amber 主色強調可測驗、Obsidian 保留 secondary

##### WP11 · Wikilinks 樣式 vs spec citation 樣式分離 [local]
- **現況**：wiki 內 wikilinks normal link 樣式（無 amber、無 dashed underline）
- **設計稿 03b quiz citation**：wikilinks 是「**dashed-underline mono amber**」
- **決策（採我建議）**：**分開**——
  - wiki 內 wikilinks（連到別 page）= normal underline link（amber on hover 即可）
  - quiz citation wikilinks（指向源 page 引用）= dashed-underline amber（強調 citation 屬性）
  - 兩種 context 不同樣式不衝突

##### WP12 · 缺 page metadata [→ WP2]
- 由 WP2 cover；不獨立修

##### WP13 · codebus:// scheme vs Obsidian `[[wikilink]]` 雙向相容性 [verify] [open]
- codebus app 內：`codebus://wiki/<path>` scheme link
- Obsidian 開同 page 時：是否能 navigate？
- **要驗**：
  - a. 雙向都能 navigate（codebus 自動雙重渲染 dual format）→ OK
  - b. Obsidian 內 codebus:// scheme 失效 → README 主張「Obsidian-compatible」破功
- **狀態**：超出 design audit 範圍但要記錄；可能影響 README brand promise

### Wiki empty Gap（完全沒 page 時）

> `WikiTab.tsx:50-64` 顯示一行純英文 hint「No wiki pages yet — run a goal to start documenting」，全屏置中、無 hero、無 CTA、無視覺結構。

##### WK-EMPTY-1 · 完全 empty state 視覺超陽春 [local]
- 純一行 hint，跟 04b Lobby empty / Goals empty 視覺密度落差大
- **修法（同 R3 Y 方向）**：
  - `📂` (lucide Folder 56px) hero icon
  - h-empty「還沒有任何 wiki page」
  - 副標「跑一個 goal，codebus 就會邊讀邊把 mental model 整理成這裡的明信片」
  - CTA：amber **`→ 跑一個 goal 開始`** button（auto setActiveTab('goals') + 強烈建議再 open NewGoalModal）
- **CTA label 決策（2026-05-26 update）**：原本「→ 跳到 Goals 開始」太隱晦——user 在 Wiki tab 不一定建立「wiki 由 goal 產生」的 mental model；CTA 文案要**直接揭露**這層因果關係。「→ 跑一個 goal 開始」一次完成「告訴 user 怎麼做」+「跳到 Goals 」兩件事

##### WK-EMPTY-2 · 文案 i18n + 改善 [i18n Cat B]
- 現況「No wiki pages yet — run a goal to start documenting」hard-code
- 翻譯：跟著 WK-EMPTY-1 新文案
- en 候選：「No wiki pages yet — run a goal and codebus will start writing」

##### WK-EMPTY-3 · 缺 CTA [shared with WK-EMPTY-1]
- user 在 Wiki empty 沒接續行為的出口
- 跟 WK-EMPTY-1 一起做，CTA = 跳 Goals tab

---

## 03 · Quiz

> 設計稿覆蓋 03a Pending / 03b Reviewing 兩個逐題 view；codebus 自加了多個額外 view（empty / scope confirmation / generation log / completion summary / history list / review-all），這些是 design 灰區。
>
> 整個 quiz flow 本質是 **multi-step wizard**（topic 輸入 → planning → scope confirm → generation → 逐題答題 → completion summary → 進 history → review-all），跟 Goal 的一次性 modal 性質完全不同。

- **現況截圖**：
  - `.quiz-empty.png`（quiz empty state）
  - `.quiz-new-click.png`（按 + New quiz 後 inline form）
  - `.quiz-generating.png`（planning 階段）
  - `.quiz-confirm.png`（scope confirmation step）
  - `.quiz-pending.png`（03a Q3 還沒選）
  - `.quiz-answered-correct.png`（03b Q1 答對）
  - `.quiz-answered-wrong.png`（03b Q2 答錯）
  - `.quiz-completed.png`（quiz 完成 summary）
  - `.quiz-list-populated.png`（quiz history 有資料）
  - `.quiz-reentered.png`（點 history row 進 review-all view）
- **規格來源**：`design-handoff/README.md` § `03a / 03b · Quiz`
- **現況實作**：
  - `codebus-app/src/components/workspace/QuizTab.tsx`（list + new-quiz inline form + state 切換）
  - `codebus-app/src/components/workspace/QuizGenerationLog.tsx`
  - `codebus-app/src/components/workspace/QuizAnswering.tsx`（03a Pending 答題）
  - `codebus-app/src/components/workspace/QuizReview.tsx`（03b Reviewing + completion summary + review-all）

### 03 哲學決策 · Quiz 是 multi-step wizard（philosophy）

**決策（harry 確認 2026-05-26，修正我前面誤推 modal 的判斷）**：

- Quiz flow = 6+ 步 wizard（input topic → planning → scope confirm → generation → 逐題答題 N 輪 → completion summary）
- Goal flow = one-shot（modal 填表 → submit → goal 自跑）
- **本質不同、應該用不同 pattern**：
  - Goal → Modal（NewGoalModal 已實作 ✅）
  - **Quiz → Fullscreen wizard view**（進入專屬 view，類似 GoalDetail 那種「進入專屬 view」）
- 我之前推 modal 是錯誤校準（基於「同 + 按鈕應該一致」直覺，但 multi-step 跟 one-shot 本質不能強求一致）

### Quiz wizard view 統一規格（design v1 ack 2026-05-26）

- 點 `+ New quiz` → topbar `+ New quiz` **hidden 不是 disabled**（design v1：disabled 創造 "what if I click that?" 困惑；hidden unambiguous，同 "Goal Running 時不顯示 + New Goal" 規則）
- header 變「New quiz」 + step indicator dots + label（規格見 QC2 design v1 spec）
- content area 是 wizard step body
- 每步可 Cancel / Back
- **退出回 Quiz history**（不退到 Lobby / Goals empty，continuity 重要）
- **4 個 step（design v1 lock）**：`topic → scope → generating → answering`
- **answering step 不再切 step-dot**，header 改顯示 `Q3/5`（per-question 是 live concern）

### Quiz scope-confirm step（FLAGSHIP surface · design v1.5 spec lock）

> walkthrough-decisions.html §03 把 Design A / B 雙版都 spec 出來。**對齊點 6 決議：走 Design A**（with match metadata · trust-builder 厚度厚）。Design B 仍保留為 fallback。

**共用結構**（兩版都長這樣，walkthrough-decisions §3.1）：

| 區塊 | 內容 |
|---|---|
| Header strip | `← Quiz history` + title「New quiz」+ dots `●●○○` + label `Step 2 / 4 · Scope` + 右側 Cancel |
| Topic banner | `🎓 quiz 主題：<topic>`（read-only，**對齊點 8 決議：用 🎓 不用 🎯**） |
| Instruction line | Design A / B 文案不同 |
| Scope alert（conditional）| 2-7 pages sweet spot；<2 或 >7 amber warn 不擋路 |
| Page list | 每頁 row · checkbox + path + title + summary/reason tags · default checked · 可逐項 uncheck |
| Footer | 左 `5 pages selected` live count；右 `[ 重新規劃 ]` secondary + `[ 確認 · 開始出題 ]` amber primary |

**Design A spec**（with match metadata · 推薦版）：

- **Backend contract**（codebus-quiz skill output schema 加 reason field）：
  ```ts
  scope_proposed: {
    pages: Array<{
      path: string;
      title: string;
      snippet: string;     // 第一段或 H2 摘要
      match_score: number; // 0-1
      matched_keywords?: string[];
      linked_from?: string;   // 「被誰 wiki link 引用」
      authoring_goal?: string;
      last_updated_at?: string;  // ISO
    }>;
  }
  ```
- **Reason tag 3 色**（walkthrough-decisions §3.2）：
  - **Amber tint** — `matched · 專案 · 目的`（直接 keyword match、最強 signal）
  - **Neutral** — `linked from · 桌面工作台` / `authored · 完整研究專案目的` / `updated · 12h ago`（間接 signal）
  - **Neutral · low match**——`low match · 0.3`（score < 0.5 自動加，提示 user 特別判斷）

**Design B fallback spec**（無 metadata）：
- 只需 `{ path, summary }`，snippet 改 3 行 clamp（A 是 2 行）
- 沒 reason tags、user 靠 snippet 自己判斷對不對題
- Trust-builder 來源：「這頁長什麼樣」而非「為什麼選這頁」

**Scope alert spec**（兩版共用，walkthrough-decisions §3.5）：
- `scope · few`「只挑到 1 頁，quiz 可能會太單薄。試試擴大主題、或先『重新規劃』看看 codebus 有沒有忽略相關頁面。」
- `scope · many`「挑到 9 頁，問題會很雜。建議 uncheck 一些次要的、或拆成多個 quiz 各自聚焦。」

**State machine**（walkthrough-decisions §3.6）：

| 觸發 | 動作 |
|---|---|
| 進入此 step | planning step 完成 emit `scope-proposed` 後到 |
| 「確認 · 開始出題」 | 送出 `selected_paths: string[]` · 進 generating step |
| 「重新規劃」 | 退回 planning step · LLM 重新規劃；**prompt 帶 prior selection + uncheck 過的 path 當 negative signal**（**對齊點 7 決議：接受**，codebus-quiz skill 加 `previous_rejected_paths: string[]` input） |
| 「Cancel」 | 退回 Quiz history · 不存 state |
| uncheck 全部 page | 「確認」disable · footer count 標紅 `0 / 5 pages selected · pick at least 1` |
| refresh / app 重開 | state lost · 回 Quiz history（這步不持久化、modal 邊界） |

### Quiz empty Gap

##### QE1 · Header「Quiz history」+ subtitle [local]
- 現況：純英文 `Quiz history`
- **修法（採我建議）**：`Quiz` h1（jargon 保留）+ subtitle「驗證自己有沒有看懂 wiki」（對齊 04b Quickstart step3 文案）

##### QE2 · Empty hint 純英文 hard-code [i18n Cat B]
- 「No quizzes yet — start one with + New quiz」→ 翻成「還沒有 quiz —— 用 + New quiz 開始」

##### QE3 · Empty 視覺陽春 [shared with WK-EMPTY-1 / R3]
- 跟 R3 Y 方向：lucide `GraduationCap` hero + h-empty + 副標 + content-area amber CTA
- 文案：「還沒做過任何 quiz」/「考驗一下你看 wiki 看懂了多少」

##### QE4 · `+ New quiz` 位置 [shared with GP2 / R6]
- 現況在 topbar；該在 content header row
- 跟 GP2 / R6 共用 — Goals/Quiz/Wiki 都採同 layout pattern

##### QE5 · topbar 分隔線 [shared G5]

### Quiz new-quiz flow Gap（按 + New quiz 之後）

##### QNEW-1 · topbar `+ New quiz` 沒反映 state（持續顯示原樣）[bug]
- 修法：走 Quiz wizard fullscreen view，topbar `+ New quiz` 進 wizard 後隱藏

##### QNEW-2 · header「Quiz history」 在 wizard state 沒更新 [bug]
- 修法：wizard view 下 header 顯示「New quiz · Step X/N」

##### QNEW-3 · 「Start」 純英文 [i18n Cat B]
- `QuizTab.tsx` Start button 補進 Cat B

### Quiz scope confirmation Gap（QC 系列）

##### QC1 · Header 沒反映 step [shared QNEW-2]

##### QC2 · Wizard step indicator [local] [design v1 spec ack]
- **規格**（design v1 spec 2026-05-26）：**dots + label 並排，左對齊在 header strip**
- **Dots**：7px 圓
  - `done` step：filled-fg-tertiary
  - `current` step：filled-amber + outer ring (`accent-tint`)
  - `pending` step：ring-only `border-strong`
- **Label format**：`Step <b>2</b> / 4 · <b>Scope</b>`（current 數字 + step name 用 fg bold）
- **answering step 切換**：dot indicator 消失、改顯示 `Q3 / 5`（per-question 是 live concern）
- 位置：在 header strip 替代原本 `Quiz: <scope>` mono 標題的位置

##### QC3 · wiki page list 純 mono 無視覺結構 [polish, low priority]
- 可加 file icon prefix + hover preview；不是 bug

##### QC4 · 「重新規劃」/「確認」 button 樣式 [local]
- 兩 button 都 secondary；「確認」應該 amber primary（next-step CTA）
- 修法：「確認」amber、「重新規劃」secondary

### Quiz generation log Gap（QG 系列）

##### QG1 · Header 沒反映 state [shared QNEW-2]

##### QG2 · 副標「Planning quiz scope...」/「Generating questions...」純英文 + 該動態 progress [i18n Cat B + shared with LOI-1]
- 跟 LoadingOverlay LOI-1 同 idea
- backend 是否有 quiz progress event 待驗

##### QG3 · activity stream 跟 Goal Running 共用 UI [shared confirm]
- 確認 X1 (shell wrapper) / W4 (visual grouping) / W5 (stream tail) / W6 (stream log card) **都自動 cover quiz generation**

##### QG5 · 缺 Cancel button [bug, missing feature]
- Goal Running 有 Cancel；Quiz generation 沒
- 修法：補 Cancel，跟 Goal Running 同 pattern（header right）

##### QG6 · 缺 elapsed / tokens 計數 [local]
- Goal Running 有「Xs elapsed · X tokens」；Quiz generation 沒
- 統一改：跟 Goal Running 同 layout

##### QG7 · 缺 brand banner [open]
- Goal Running 有「🚌 來囉來囉~」「🎯 任務目標：...」 banner；Quiz 直接從 thought block 開始
- 建議補：「🎓 quiz 主題：xxx」當首 banner

##### QGEN1 · `[CODEBUS_QUIZ_NO_VALIDATE]` 內部 marker tag 曝光給 user [bug]
- `🤔 [CODEBUS_QUIZ_NO_VALIDATE] codex sandbox cannot run quiz structure validation` raw 顯示
- 修法：`ActivityStreamItem` render thought block 時 detect `[CODEBUS_*]` pattern → 轉成 user-friendly 文案 或過濾
- 候選文案：「codex 沙箱無法跑 quiz 結構驗證，跳過此步」

##### QGEN2 · Codex shell wrapper UTF-8 encoding 問題 [bug] [shared X1 姊妹議題]
- LLM 自述「terminal encoding is mangling CJK text. I'm re-reading the same wiki/ files with explicit UTF-8...」
- 證明 codex sandbox / powershell wrapper 預設**不是 UTF-8**，CJK 內容被亂碼
- LLM 自己 detect 並 retry → 浪費 token + 時間
- **Root cause**：codex 包 `powershell.exe -Command` 啟動時沒設 `chcp 65001` 或 `[Console]::OutputEncoding = [System.Text.Encoding]::UTF8`
- **嚴重度高**：每次 goal / quiz 跑 CJK vault 都會撞
- **超出 design audit 範圍但要記錄**

##### QGEN3 · LLM 思考輸出語言混雜（前英後中）[open] [non-audit-scope]
- 第一段 thought block 英文、後段中文
- prompt 工程議題、不是 UI gap

### 03a Pending Gap（逐題答題前）

##### QP1 · Submit button disabled→enabled 變化 [verify]
- 截圖未選 choice 時 Submit grey disabled
- 設計稿 spec: 選了後 Submit 應變 amber primary + 旁邊 `⏎ to submit` hint
- 結算階段實測變化

##### QP2 · Choice row 缺 letter chip + radio circle [shared QA4]
- 設計稿 `[A] (10/mono boxed key) · radio · label · optional tag`
- 現況 A)/B)/C)/D) 行首寫死、無 chip 無 radio
- 修法：每 row 補 letter chip + radio

##### QP3 · 缺 `⏎ to submit` keyboard hint [shared QA9]
- footer 加 kbd hint 列

### 03b Reviewing Gap（答完一題）

##### QA1 · Header「Quiz history」沒顯示 quiz scope [shared QNEW-2]
- 設計稿：header 是「Quiz: <scope>」 (mono) + 右側 `Q3 of 5` counter
- 修法：跟 wizard fullscreen 一起改

##### QA2 · `Question X of 5` 位置 + 文案 [local + i18n]
- 設計稿：counter 在 header strip 右側、`Q3 of 5` 簡潔
- 現況：content 區純文字「Question 1 of 5」 hard-code
- 修法：搬 header right + 改「Q1 of 5」

##### QA3 · 問題缺 `Q1.` mono 前綴 [local]
- 設計稿：`Q3.` mono dim 18px + question 22px/600

##### QA4 · Choice rows 缺 letter chip + radio [shared QP2]

##### QA5 · Wrong / fade state 不符 spec [local]
- 設計稿：non-selected non-correct choices fade 55% opacity
- 現況：其他 choices 視覺正常、沒 fade
- 修法：補 opacity 處理

##### QA-WRONG-1 · 缺 `your answer` / `correct` tag [local]
- 設計稿：wrong row red border + ✕ + red `your answer` tag；correct row green border + green check + green `correct` tag
- 現況：有 border 顏色、**沒 tag 標籤**
- 修法：補 inline tag

##### QA-WRONG-2 · 「Incorrect」/「Correct」 大字 vs spec 小 tag [verify]
- 設計稿用 inline `correct` / `your answer` 小 tag
- 現況用大字「Correct / Incorrect」獨立區
- 可保留 codebus 大字風格（強調感更強），但 tag 該補（cover spec）

##### QA6 · Explanation 不是 spec 的 citation blockquote ⭐ [local] [design v1 spec ack]
- 設計稿：「**Citation blockquote**: 2px amber left border, bg-raised fill, amber `"` opening glyph, cited quote, dashed-underline mono wikilink in amber」
- 現況：純 text 段落 + inline wikilink、**完全沒 blockquote 樣式**
- **規格（design v1 demo 確認 2026-05-26）**：
  - 2px amber left border + `bg-raised` fill + `border-radius: 0 4px 4px 0`
  - 開頭 amber `"` opening glyph（Georgia serif, 22px, line-height 0, vertical-align -6px）
  - 內部 wikilink = **dashed-underline mono amber**（`cite-link` 樣式）
- **「← Back to wiki page」 link 用 plain 樣式，不是 citation 樣式**（navigation 不是 provenance）

##### QA7 · wikilink 樣式不符 [shared WP11]
- 設計稿 03b citation wikilink = amber + dashed-underline + mono
- 現況淺藍 normal link
- 跟 WP11 連動：wiki 內 normal、quiz citation amber dashed

##### QA8 · `Next` button + 缺 `← Back to wiki page` link [local]
- 設計稿 footer：左 `← Back to wiki page` + 右 amber `Next: Q4 →`
- 現況：單一 `Next` secondary button
- 修法：
  - `Next` 改 amber primary、顯示下題編號
  - 加「← 回 wiki page」link（跳 citation 來源）

##### QA9 · 缺鍵盤 hint [shared QP3]
- 設計稿：Pending `A B C D` to select + `⏎` submit；Reviewing `→` advance

### Quiz completion summary Gap（QF 系列，design 灰區）

##### QF1 · 完成 summary 視覺 [local] [design v1 spec ack]
- 設計稿 v1 沒覆蓋；現況純文字
- **規格（design v1 spec 2026-05-26）**：
  - **Hero icon 56px**：
    - **Fail** = lucide `XCircle` **red 色**（`--error`，**不是 amber**——fail 是 definitive outcome 不是 interrupted state）
    - **Pass** = lucide `CheckCircle2` **green 色**（`--success`）
  - h-empty：「沒通過 (40%)」/「通過了 (88%)」
  - 副標 fg-secondary：「正確率 40%，未達 69% 門檻」/「正確率 88%，超過 69% 門檻」
- **Actions（design v1 update：降到 2 個 button）**：
  - amber primary：「重做此份」
  - secondary：「看錯題」（pass case 改成「看過程」 = generation log）
  - **「← 回 history」 link** 放**左上角** page corner（**不是** action button）
  - **理由**（design v1）：「user 剛答完 5 題、累了；3 個 button 太多；把 navigation 降回 link」

##### QF2 · 「Failed (threshold 69%)」純英文 hard-code [i18n Cat B]
- 已記在 cross-cutting i18n Cat B（`QuizAnswering.tsx:142`）
- 翻譯：「未通過（門檻 69%）」/「通過（門檻 69%）」

##### QF3 · 缺 action buttons [local]
- 現況是死路—— user 必須按 「← History」 才能繼續
- 修法：補 retry / review / back actions（QF1 已涵蓋）

### Quiz history list Gap（QL 系列）

##### QL1 · Row title 用 hash ID 而非 user 給的 topic ⭐ [bug] [HIGH PRIORITY, design v1 認可]
- 現況：title 顯示「topic-a7fb67fc」 hash ID；副標才是真 topic「專案目的」
- **問題**：user 用 「專案目的」 起 quiz、list 用內部 ID 當主標題 — 認不出
- **Design v1 評語**：「Clean catch. Show users the topic they typed, not the topic's internal ID — engineers invisible / users obvious. **Prioritize this.**」
- **修法**：title 改成 user 給的 topic「專案目的」、hash ID 拿掉或移到副標

##### QL2 · 時間戳格式 raw ISO [local]
- 現況「2026-05-25T16-53-17Z」
- 修法：跟 GP7 一樣走 `t("common.minutesAgo")` etc → 「34 分鐘前」

##### QL3 · 「fail」純英文 + 無色彩 [i18n + local]
- 現況：「5/5 · 40% · fail」 mono 灰色
- 修法：加 status dot 或 colored tag（fail = red、pass = green、跟 X4 status token 共用）

##### QL4 · 「5/5」 含義不直覺 [verify]
- 「5/5」可能是答完 5 題（/5 total）；40% 是得分率；fail 是結果
- 格式不直覺；建議「2 / 5 答對 ・ fail」 mono
- 結算階段驗

##### QL5 · 缺 kebab / hover [shared GP6 / G-04a-1]
- 跟 Goals list / Vault card 同 pattern 缺失

##### QL6 · header「Quiz history」 + 右上 `+ New quiz` 位置 [shared GP2 / R6 / QE4]

### Quiz review-all Gap（QR 系列，design 灰區）

> 點 history row 進入「全題顯示」review 模式——一次看完整 quiz 所有題目、答案、explanation。**這是 codebus 自加的 valuable feature**。

##### QR1 · review-all view 是 codebus valuable feature [design 灰區]
- 設計稿 03a/03b 只覆蓋逐題 view，**沒覆蓋這個 review-all view**
- 保留現況設計（valuable）

##### QR2 · Header action layout [local]
- 「← Back to history」 (英) + amber `重做此份` + secondary `看過程`
- 「看過程」需要解釋——是看 generation log？應該改「看 quiz 怎麼生的」更白話
- 「← Back to history」英文 → i18n Cat B 補洞

##### QR3 · 「Your answer: B · Correct answer: B」 純英文 [i18n Cat B]
- 翻譯：「你的答案：B ・ 正解：B」

##### QR4 · 「Failed (threshold 69%)」 [shared QF2]

##### QR5 · 「← Back to history」 [i18n Cat B + shared with S1 系列]

##### QR6 · 整題堆疊缺視覺分隔 [local]
- 每題之間沒明顯分隔
- 修法：題目間加 hairline 或 spacing

### 03 共用議題 update

- **wizard fullscreen view 統一**：QE / QNEW / QC / QG / QA 系列大部分 header / state 問題都靠 wizard pattern 一起解
- **Letter chip + radio 統一**：QP2 + QA4 一起做
- **i18n Cat B 補洞集中**：QE2 / QNEW-3 / QG2 / QGEN1 / QF2 / QL3 部分 / QR2 / QR3 / QR5 / QA2 / QA9 / QP3
- **X4 status token 套到 quiz**：QL3 fail = red、QF status 配色跟 GP5 三態系統共用
- **QGEN2 UTF-8 encoding** 屬 codex provider integration bug，超出 design audit、獨立議題

---

## 06 · Settings Modal

- **現況截圖**：
  - `codebus-app/scripts/.settings-top.png`（2026-05-25, Azure profile）
  - `codebus-app/scripts/.settings-system.png`（2026-05-25, System profile）
  - `codebus-app/scripts/.settings-bottom.png`（2026-05-25, modal 下半部）
- **規格來源**：`design-handoff/README.md` § `06 · Settings Modal`（spec **沒詳細展開內容**，只說 "Global preferences modal"）
- **現況實作**：
  - `codebus-app/src/components/settings/SettingsModal.tsx`（主框，**已 i18n 完整**）
  - `codebus-app/src/components/settings/EndpointSection.tsx`（Claude endpoint，**未 i18n**）
  - `codebus-app/src/components/settings/CodexEndpointSection.tsx`（Codex endpoint，**未 i18n**）
  - `codebus-app/src/components/settings/SetKeyDialog.tsx`（API key 設定 dialog，**未 i18n**）

### 觀察：i18n 痛點精確命中

完整 sweep 後確認 cross-cutting i18n Cat A 範圍**精確**：

- ✅ **SettingsModal 主框**（title、AI 提供者 label、PII 區、所有 toggle、Log 路徑、Quiz 設定 slider）—— **i18n 完整**
- ❌ **EndpointSection / CodexEndpointSection / SetKeyDialog**（System/Azure profile、API key 區、所有 field label、按鈕）—— **完全 hard-code 英文**

→ cross-cutting Cat A 修法只需動三個 subcomponent 檔。

### 06 Gap

##### ST1 · `Installed · codex-cli 0.133.0` badge 英文 [i18n Cat A 補洞]
- **問題**：badge 顯示 CLI 安裝狀態 hard-code 英文
- **修法**：補進 Cat A sweep；i18n key 命名類似 `settings.codex.installed`，內含 `{version}` placeholder
- **位置**：`CodexEndpointSection.tsx`（動態 install detection 訊息）

##### ST2 · chat verb row「沿用 query」 [by design]
- **背景**：chat verb row 顯示「沿用 query (gpt-5.4-mini / low)」（無 input field）
- **設計判斷**：chat 是輕量問答、跟 query 共用 model/effort 合理
- **狀態**：**by design，不開 gap**

##### ST3 · D1 例外名單擴充 [shared cross-cutting]
- 詳見上方 `## Cross-cutting · i18n` 原則 #2 已更新
- 加入：verb names、codex effort 值、PII 行為 enum、config YAML key 名

##### ST7 · PII 掃描器 dropdown 下方 helper text 重複 [local]
- **問題**：dropdown 顯示「regex_basic · 14 條規則」，下方 helper text 又顯示一模一樣的字
- **修法**：改成 descriptive 內容例如「regex_basic 涵蓋 email / API key / IP 等 14 種 pattern」（給 user 更多 context；不要單純移除，helper text 該補資訊）

##### ST10 · 「擋圖片 / binary 讀取」label 跟 hint 邏輯反向 [local]
- **問題**：
  - label：「擋圖片 / binary 讀取」（toggle on = 啟用擋功能）
  - hint：「關閉後 agent 可 ingest 圖片 / PDF / binary 檔到 context」（toggle off = 允許 ingest）
- **影響**：認知負擔重；user 要在「擋啟用 vs 讀取允許」之間反向思考
- **修法**：改成「允許讀取圖片 / binary」反向 phrasing（toggle on = 允許），跟 hint 邏輯一致
- **注意**：實作時要**反轉 boolean default 值** 對應 UI 顯示語意；store / config.yaml 內部 boolean 可保留原語意只翻轉 UI 顯示

##### ST12 · Quiz 內容驗證 / Goal 內容驗證 hint 完全相同 [low priority] [verify]
- **問題**：兩個 toggle 顯示完全一樣的 hint「開啟會多花 verify/repair agent spawn（較慢、token 成本較高）」
- **要驗**：兩者實際行為是否真的用同一個 verifier
  - 若同 verifier：hint 一樣 OK，可標 by design
  - 若不同 verifier：hint 該分別說明
- **狀態**：低優先，等實作驗證階段確認

##### ST13 · Modal 沒 section/tab 結構 [observation, no gap]
- 現況：整個 modal 是一條 long form（Provider / PII / 行為 toggles / Log / Quiz limit 全堆疊）
- 隨功能增加會變很長；**目前可用**
- 未來若加更多 settings 再考慮分 tab（Provider / Privacy / Behavior / Quiz）
- **不開 gap**

### 06 沒有 layout gap

- Design spec 06 沒詳細展開 settings 內容，現況 layout 設計合理
- 唯一痛點 = i18n（endpoint section subcomponent hard-code 英文），由 cross-cutting Cat A 處理

---

## 05 · Cmd+K Overlay (CUT)

**決定**：2026-05-25 砍掉，不實作。

### 理由

1. 沒任何 UI 提示這個功能存在——做了等於沒人發現、沒人會 miss
2. ChatWidget 已經做了 05 想做的事（query + citations）而且更多（多輪對話、promote to goal、token display、Undo）
3. 維護兩個 chat-like UI = 長期負擔；功能 80% 重疊
4. spotlight 風格（centered + blur backdrop + ESC dismiss）的價值是「臨時聚焦感」——這可作為 ChatWidget 的一個 **mode** 而不是另一個 feature

### 後續處置

- `design-handoff/design_files/components/cmdk-overlay.jsx`、`design-handoff/README.md` § `05 · Cmd+K Overlay` 規格**不刪**，保留當作 spotlight pattern 參考
- 結算階段不為 05 開實作任務
- 若未來需要 spotlight 體驗，走 ODI-3（ChatWidget centered modal mode）


## Open Design Ideas

> 超出 design spec 但值得做的構想。每條獨立評估，跟 Gap 區分開。

### ODI-1 · 04b hero 🚌 idle motion · "Bumpy road"

- **狀態**：proposed（harry 提出 2026-05-25）
- **範疇**：只在 04b empty state 的 56px hero emoji；topbar 的小 🚌 **不**做動畫
- **動機**：04b 是首次體驗畫面（加完第一個 vault 後就不再看到），是 dev tool 准許 brand 表現的場合；強化 README 的公車敘事而不破壞工程師感
- **風味**：**Bumpy road（顛簸前進）** — 垂直 2px bob + 水平 1px shake，兩軸異步，loop 1.2-1.6s
- **約束**：
  - `@media (prefers-reduced-motion: reduce)` 必須 fallback 完全靜態
  - 不影響 layout（用 `transform`，不動 box）
  - 不加任何外部依賴，純 CSS `@keyframes`
- **不在範圍**：路面虛線、輪子動畫、進場/離場過場、其他畫面的 🚌

### ODI-2 · Fullscreen 大留白 background ambient

- **狀態**：proposed（2026-05-25，G1 修完後若 fullscreen 仍空再考慮）
- **被誰引用**：04 Lobby G1（fullscreen 大留白驗收條件）
- **動機**：1920×1080 + 100% Windows scaling 下，04b/04a hero 中央 440-640px 寬，左右兩側仍有大塊純黑——「空虛感」即使 G1 修了垂直置中還是會剩
- **scope**：
  - 極淡 dot grid pattern（透明度 3-5%、單色）填 viewport 邊緣大留白區
  - **不**用 multi-stop gradient（設計稿明文禁）
  - **不**用 glassmorphism（設計稿明文禁）
  - 候選風味：subtle dot grid（GitHub empty state 風）、faint hairlines、單一極淡 vignette
- **觸發條件**：G1 landed + harry 在 1920×1080 100% 縮放下實測 fullscreen 仍覺得空
- **不立即做的理由**：可能 G1 修完視覺密度就夠、ODI-2 是備案

### ODI-3 · ChatWidget centered modal mode（spotlight 替代品）

- **狀態**：proposed（2026-05-25，05 Cmd+K Overlay 砍掉時順手記）
- **動機**：若未來想要 spotlight 體驗（centered + blur backdrop + ESC dismiss），不重新做 05，而是讓 ChatWidget 多一個顯示 mode
- **scope**：
  - 加 setting / shortcut 切換 ChatWidget 的兩種 mode：corner panel（現況）/ centered modal（spotlight 風格）
  - centered modal mode：固定中央、背景 blur、ESC dismiss
  - 行為跟功能不變、只是 layout 變
- **觸發條件**：harry 自己用 ChatWidget 一陣子後仍覺得 corner panel 「太重」、需要 quick query 場景
- **不立即做的理由**：ChatWidget 現況可用、05 砍掉沒人 complain；speculative feature

### ODI-4 · ChatWidget collapsed 圓鈕 active goal pulse

- **狀態**：proposed
- **動機**：goal running 時 ChatWidget collapsed 圓鈕加 amber pulse dot，提示「有事在進行」（也讓 user 知道 cancel 在 header 不是 widget 上）
- **連動 R7-2 的共構視覺契約**：Cancel 搬 header 後，ChatWidget 圓鈕同時承擔「chat 入口」+「goal status indicator」兩個職責。視覺契約：
  - 預設狀態：靜態 MessageSquare icon、`text-fg`、`bg-bg-raised`
  - active goal running：右上角 amber pulse dot (`accent-ring` 顏色，6px)、icon 不變
  - 兩態切換經由 `useGoalsStore` activeRun 偵測
- **不在範圍**：goal done / interrupted / failed 完成後的 pulse dot 行為——pulse 只在 running 期間活，goal 結束後 dot 消失（不留 unread 樣式，避免跟 chat unread 衝突）
- **polish 等級**：非 gap、視結算優先序

### ODI-5 · Settings 字號 scale（從 cross-cutting 字號區搬來）

- **狀態**：deferred（原 ODI-2，2026-05-26 重編號避免跟 ODI-2 background ambient 撞號）
- **動機**：若全 token +1 級 bump 仍有 user 反映偏小，加 user-level font scale preference
- **暫不做理由**：在「default 不夠」沒被驗證前先加 setting = 過度設計；牽動 design token system / 多 scale 測試 / snapshot 暴增
- **觸發條件**：+1 級實機一週後 harry（或未來 user）仍 complain
- 詳細規格參考 `## Cross-cutting · 字號 scale` 區的 「驗收」 條目

---

## 結算階段 · Next Steps

> 2026-05-26 寫、2026-05-26 update（吸收 design v1 reply）。
> Audit 攤完所有 gap、不寫實作；本節給結算階段排序、批次切分、依賴順序、風險評估、spectra change 建議、design walkthrough schedule、v1.1 handoff 待收。

### 高優先 critical bugs（先修）

| ID | 描述 | 影響 |
|---|---|---|
| **QL1** | Quiz history row title 用 hash ID 而非 user 給的 topic | user 認不出自己的 quiz（design v1 認可：勿延後）|
| **R7-2** | ChatWidget 圓鈕擋 RunDetailRunning Cancel button | 跑 goal 中 user 無法 cancel |
| **X1** | Codex shell wrapper extraction（`powershell.exe -Command "..."` 整列吃掉、actual cmd 被截）| 全 activity stream tool row 不可讀 |
| **QGEN1** | `[CODEBUS_QUIZ_NO_VALIDATE]` 內部 marker 直接 raw 顯示給 user | 像 bug 字串 |
| **QNEW-1/2** | 按 `+ New quiz` 後 topbar button 沒消失、header 沒更新 | flow 認知衝突 |
| **GP8** | Running row 沒 stream tail（design v1 強烈表態升 priority）| ambient awareness 嚴重不足；running vs done 視覺相同 |

### 批次工作建議（按改動性質分群）

#### 批次 1 · i18n sweep（一次清完三類）

- **Cat A**：`settings/EndpointSection.tsx` + `CodexEndpointSection.tsx` + `SetKeyDialog.tsx`（最大痛點）
- **Cat B**：workspace 系列（`QuizAnswering` / `QuizReview` / `QuizTab` / `NewGoalModal` / `ChatInput` / `GoalsTab` / `RunDetailRunning` / `RunDetailCancelled` 各 back button 等）
- **Cat C**：aria-label / title 補洞（`ui/dialog` / `ChatWidget` / `WikiTab` / 3 處 `title="Page not found"`）
- **Cat D**：保留英文但走 i18n bundle（一致化）
- 連動條目：G-copy-1 / G-copy-2 / S1 / W10 / D-* / QE2 / QNEW-3 / QG2 / QGEN1 / QF2 / QL3 / QR2-5 / I1 / ST1 / WK-EMPTY-2 / WP10 / QP3 / QA2-9 / WK7

#### 批次 2 · 字號 scale +1 級 bump（design token 層）

- Tailwind config / globals.css token 改字號
- 全 component sweep hard-code `text-[Npx]` 換 token
- snapshot test 重生
- **要先做**：很多後續視覺驗收依賴新字號（border 對比、layout 密度）

#### 批次 3 · Border token 對比度校準

- G5 / S2 / R1 / WK6 / WP7 / QE5 等所有 hairline 議題共用
- 換 `--border` 預設值或新增中等對比 token
- **建議跟批次 2 同時做**，因為 visual snapshot 都會重生

#### 批次 4 · Status 三態 token + indicator 統一

- GP5 / X4 / QL3 / QF1
- 三態 token：`--success` (done green) / `--warn` (interrupted amber) / `--error` (failed red)
- 套用：Goals list dot / Goal Detail header pill / Quiz history fail tag / Wiki page badge

#### 批次 5 · 共用 component 抽出

- **`<CollapsibleStreamLog>`** — 用於 W6 / D3 / I2-I5（02a / 02b / 02c 三個 detail state）
- **`<HoverKebabMenu>`** — 用於 G-04a-1 / GP6 / QL5（vault card / goal row / quiz history row）
- **`<EmptyStateHero>`** — 用於 R3 / WK-EMPTY-1 / QE3 / QF1（Goals / Wiki / Quiz empty + completion）—— 同一個 hero + h-empty + 副標 + CTA pattern

#### 批次 6 · Layout reorg：CTA 搬進 content header

- R2 / R6 / GP1 / GP2 / QE4 / QL6
- 全 workspace tab 統一 content header row（h1 + subtitle + CTA + shortcut chip）pattern

#### 批次 7 · Sidebar 改造

- S3（drop section label）/ S4（emoji prefix）/ S5（mono count）/ S6（active amber bar）/ S7（sidebar footer + drop refresh + ⌘K chip）/ F1（BottomStrip 改 Lobby-only）

#### 批次 8 · 02 Goal Detail 結構性大改

- 02 哲學決策 hybrid 方案 + W2 / W3 / W4 / W5 / W6 / X3 / X1
- Cancel 搬 header（解 R7-2 collision）
- timeline visual grouping
- stream log card 拆共用 component（批次 5）

#### 批次 9 · Quiz Wizard fullscreen view

- 03 哲學決策 + 全 QE / QNEW / QC / QG / QA 系列 layout 統一
- `QuizTab.tsx` 改 wizard view + state machine
- Letter chip + radio (QP2 + QA4)
- Citation blockquote (QA6) + wikilink 樣式 (QA7 / WP11)

#### 批次 10 · Lobby empty state polish

- G1 / G2 / G3 / G4 / G5 / G6 / G7（除了 G4/G5/G6 屬批次 3）
- + ODI-1 hero motion（Bumpy road）

#### 批次 11 · Wiki tab polish

- WK1-9 + WP1-13 + WK-EMPTY-1-3
- 5 buckets icon system (WK5)
- page metadata (WP2 / WP12)

#### 批次 12 · 04a Vault 詞拿掉 + kebab

- G-copy-2 / G-04a-1（待批次 5 `<HoverKebabMenu>` 完）

### Dependency 順序（關鍵約束）

```
批次 2（字號）→ 批次 3（border）→ 視覺驗收要等這兩個
                              ↓
                批次 1（i18n）── 可平行做 ──→ 批次 4（status token）
                                              ↓
                  批次 5（共用 component）─── 預先抽出 ──→
                                              ↓
        批次 6（CTA 重排）→ 批次 8（02 大改）→ 批次 9（quiz wizard）
                ↓
        批次 7（sidebar）/ 10（lobby）/ 11（wiki）/ 12（vault 詞）
```

### Risk Assessment（會引發 regression 的批次）

| 批次 | 風險 | mitigation |
|---|---|---|
| 1 i18n | locale switch 行為 / 動態插值 | vitest 全 i18n key 跑一遍 + 手動 zh/en 切換 smoke |
| 2 字號 | 所有 visual snapshot 失效 | 重生 snapshot；macOS retina + Win 100% + 4K 150% 三基準看一遍 |
| 3 border | 同上、特別是 card / panel 邊緣 | 同上 |
| 4 status token | Goals list / Goal Detail / Quiz history 三處渲染需重測 | E2E 三 state goal + quiz pass/fail flow |
| 5 共用 component | 抽錯介面導致三處呼叫 site 全 break | unit test 共用 component + 三 detail state E2E |
| 8 02 大改 | timeline 結構大改可能影響 activity stream parsing | E2E 跑完整 goal flow，看每階段 banner 都對 |
| 9 quiz wizard | state machine + URL state 持久化、user 中途 cancel 不能留垃圾 quiz | E2E quiz happy path + cancel mid-step path |

### 建議 spectra change 切分

| change 名稱 | 包含批次 |
|---|---|
| `i18n-sweep-cat-a-b-c-d` | 批次 1 |
| `design-token-typography-and-border` | 批次 2 + 3（一起做避免重複 snapshot） |
| `status-three-state-token` | 批次 4 |
| `extract-shared-components` | 批次 5（先抽 component、empty wrapper、待後續批次填內容） |
| `workspace-content-header-row` | 批次 6 |
| `workspace-sidebar-rework` | 批次 7 |
| `goal-detail-hybrid-timeline-and-cancel-move` | 批次 8 |
| `quiz-fullscreen-wizard-view` | 批次 9 |
| `lobby-empty-polish` | 批次 10 |
| `wiki-tab-polish` | 批次 11 |
| `vault-terminology-removal` | 批次 12 |
| `loading-overlay-live-progress`（獨立、超出 design audit）| LOI-1 |
| `chatwidget-icon-and-pulse`（小批，bundle 修法）| R7-1（icon）+ ODI-4（pulse） |
| `codex-shell-wrapper-extract-and-utf8`（codex provider 整合） | X1 + QGEN2 |

### Deferred / Skipped（不在這波做）

- **05 Cmd+K Overlay**：CUT，永久不做
- **G-04a-2 system chrome（frameless）**：擱置，需獨立大工程
- **ODI-1 / ODI-2 / ODI-3 / ODI-5**：polish/idea，視結算後優先序（**ODI-4 被 design v1 升 spec、不在 deferred**——是 ChatWidget 圓鈕的 running ambient indicator 規格的一部分）
- **GP9 重複 goal 視覺區分**：design 灰區，等 user 抱怨再評
- **WK9 wiki search / filter**：future
- **ST12 / ST13 settings 議題**：低優先 / observation
- **QGEN3 LLM 語言混雜**：prompt 工程議題、非 UI gap
- **R7-1 MessageSquare 換 icon**：**closed**——design v1 pushback 採納 keep 💬，不換

### Blocked by design v1.1 handoff（等下週末 mocks）

> Design v1.1 將交付以下 view 的 formal mock，動工前要等。動了會 rework 機會大。

- **LoadingOverlay live progress 細節**（LOI-1）—— design 會給 step 切分視覺 + 過場 spec
- **02c Interrupted 正式 spec**（I1-I5）—— rename 可先做、layout 等 mock
- **Quiz wizard 4 步**（QC / QG / QA / QF 系列）—— 等 v1.1 才動 layout；wizard fullscreen view 進入點可先做
- **Wiki page reader**（WP 系列）—— 等 v1.1 完整 spec；metadata bar / footer hint 可先做（規格已 confirm）
- **ChatWidget 規格**（含 centered modal mode ODI-3）—— 等 v1.1，但 R7-2 Cancel 衝突先修

### Design walkthrough · CANCELLED（2026-05-26）

原本排 3 場 walkthrough（CJK 30min / activity stream 20min / scope-confirm 45min），但 design 主動寫了 `walkthrough-decisions.html` 把三段 spec 全部 async lock 死、8 個 ⚠ 對齊點全 async 答完——**整批 walkthrough 取消**。

Design 下週末直接交 v1.1 mock，期間我們可平行動可動的批次。

### 實作 sequencing（不被 v1.1 blocked，2026-05-26 lock）

Phase 1 / 2 / 3 ... 是依賴順序、不是 sprint boundary（solo dev 不適合 sprint）。看 phase 知道**下一步 pick 哪個 change**。

#### Phase 1 · Critical bugs（立刻動、低風險）

| ID | 工作 | 動哪 |
|---|---|---|
| QL1 | Quiz history row title hash → user topic | `QuizTab.tsx` 列表 render |
| X1（frontend 端）| Codex shell wrapper extraction | `ActivityStreamItem.tsx:187` `extractInnerCommand` helper |
| QGEN1 | `[CODEBUS_*]` internal marker filter | `ActivityStreamItem.tsx` thought block render |

→ spectra change：`critical-bugs-ql1-x1-qgen1`（三合一、改動小、互不衝突）

#### Phase 2 · Design token 基礎（本週、後續視覺驗收靠這個）

- 字號 +1 級 bump：13→14 body / 11→12 meta / 10→11 micro / heading 全升一級
- Border token promote：`--border` 從 `#1f1f1f` → `#2a2a2a`，原 `#1f1f1f` 限定 in-card row separator
- `<SectionLabel>` 共用 component：default 變體（amber 2px bar + 12px/500/fg-secondary）+ `--caps` 變體（11px / 0.08em / fg-tertiary, wiki taxonomy 用）
- 影響：Tailwind config / `globals.css` / 全 component sweep `text-[Npx]` hard-code / snapshot test 重生

→ spectra changes：`design-token-typography-and-border` + `section-label-component`

#### Phase 3 · i18n + status token（可並行）

- i18n Cat A：settings endpoint subcomponents（`EndpointSection` / `CodexEndpointSection` / `SetKeyDialog`）—— 最大痛點
- i18n Cat B：workspace hard-codes（`QuizAnswering` / `QuizReview` / `QuizTab` / `NewGoalModal` / `ChatInput` / `GoalsTab` / back buttons / 其他散落）
- i18n Cat C：aria-label / title attr 補洞（`ui/dialog` / `ChatWidget` / `WikiTab` / 3 處 `title="Page not found"`）
- Cat D：保留英文走 i18n bundle（en/zh 都填英文、未來統一改用）
- 三態 status token：`--success` / `--warn` / `--error` 鎖、`<StatusPill>` component 套到 Goals list / Goal Detail header / Quiz history / QF1 completion

→ spectra changes：`i18n-sweep-cat-a-b-c-d` + `status-three-state-token`

#### Phase 4 · Layout 重排（Phase 2 token 落地後才好驗）

- CTA 進 content header（R2 / R6 / GP1 / GP2 / QE4 / QL6 共用 header pattern）
- Sidebar 重整（S3 drop label / S4 emoji prefix 🚏📂🎓 / S5 mono count / S6 active 左 amber bar / S7 settings 搬底 + drop refresh + ⌘K chip / F1 BottomStrip 改 Lobby-only）
- Lobby empty polish（G1 vertical layout / G2 amber pill / G3 num style / G6 footer token 套 / G7 density）
- Vault 詞 UI 拿掉（G-copy-2：topbar `+ 新增` / section `最近` / drag tip / G-04a-1 vault card kebab）
- 套用 `<SectionLabel>` 到 14 個位置（drop / 通用 / caps 三類）

→ spectra changes：`workspace-content-header-row` + `workspace-sidebar-rework` + `lobby-empty-polish` + `vault-terminology-removal`

#### Phase 5 · 結構性 + behavior（高 impact、有些需 backend 配合）

- **GP8 running row stream tail**（high priority、design 強烈表態；Phase 4 後立刻動）
- **W4 + X1 backend contract**：codebus-goal / codebus-quiz skill emit shell event 加 `kind: "read" | "inspect" | "mutation" | "other-*"`、frontend 2-phase cluster rendering、icon prefix、live tail 規矩
- **QNEW-1/2 wizard topbar hide**（state machine 改、`QuizTab.tsx` 重 layout 進 fullscreen wizard view）
- **ODI-4 ChatWidget pulse dot**（圓鈕右上 7px amber dot + 200ms fade-in + 跟 stream tail 同步）
- **R7-2 · Goal Cancel 搬 02a header right**（解 ChatWidget 圓鈕 collision + 跟 spec 對齊）
- **02c rename** `RunDetailCancelled.tsx` → `RunDetailInterrupted.tsx`（純 rename 可先做、full layout 等 v1.1 mock）

→ spectra changes：`goals-running-row-stream-tail` + `activity-stream-2-phase-cluster` + `quiz-fullscreen-wizard-view` + `chatwidget-pulse-and-cancel-move` + `interrupted-rename`

#### Phase 6 · 等 v1.1 mock（不要先動 layout）

- LoadingOverlay live progress（LOI-1：需 Tauri layer 接 `InitEvent` + emit `vault-init-progress`、frontend listen + 動態副標 + 6 階段切分）
- 02c Interrupted full layout
- Quiz wizard 完整 4 步 layout（topic / generating / completion；scope-confirm 已 spec、可先做 Phase 5 範圍）
- Wiki page reader 新版（metadata bar / footer hint / 整體 layout 細節）
- ChatWidget centered-modal mode（ODI-3）

→ design 下週末交 v1.1 mock 後再開 spectra change

### 在等 v1.1 期間可動的批次彙總

Phase 1 + Phase 2 + Phase 3 + Phase 4 + Phase 5（除了 W4 backend 部份要 codebus-goal/quiz 改）= **8 個 spectra change 可獨立進行**，總工程量大約 1-2 週 solo dev。

### 給 design team 的 questions（已寫進 FEEDBACK.md → design v1 已回）

**Status: closed**——10 條 Open Questions 全 inline 回 + 採納（A 採 design pushback 保留 💬；B/C/D 全 ack）。詳見文首 `## Design v1 Reply Status` 摘要。

### 驗收清單（landed 後逐項過）

- [ ] 1920×1080 100% Windows + **macOS retina baseline + 4K 150% scaling**（design v1 補）三基準視覺看過
- [ ] zh 跟 en locale 切換無 hard-code 字串遺漏
- [ ] D2 follow-up notes 跑 5 種不同 goal 看品質（**empty case 嚴格 hide section、design v1 加嚴**）
- [ ] LO-4「3-15 秒」實測（vault init 平均時間）
- [ ] WP13 Obsidian-compatibility 雙向 wikilink 驗證
- [ ] QP1 Submit button disabled→amber 變化視覺
- [ ] QL4「5/5」格式對 user 是否直覺
- [ ] G-04a 驗收：path overflow 超長 path ellipsis 行為
- [ ] **CollapsibleStreamLog localStorage sticky 驗**（X2 design v1 補；切換 vault / restart app 後展開狀態 persist）

---

*Audit 主體結束。後續實作依本 Next Steps 排序進行；任何新發現的 gap、決策變更、design v1.1 handoff 收到後的內容，必須先在這份 doc append、不能口頭傳承。*
