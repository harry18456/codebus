## Context

2026-05-27 archived change `lobby-holistic-refresh` 把 ODI-1「Bumpy road（顛簸前進）」設計提案落 spec、AUDIT brand motion lock 同步鎖定「Idling in place」mood（2px bob / 1px shake / 1.4s / 無 rotation）。實作 `codebus-bus-idle-y` / `codebus-bus-idle-x` 兩支 keyframe verbatim 對齊 spec、test `EmptyState.test.tsx:63` 只斷言 `.codebus-bus-idle` class 存在、不斷言 keyframe 名 / 數值。

2026-05-28 v1.1 implementation acceptance pass、user 親自試走 04b empty state，在 1920×1080 100% scaling 下反饋「動的幅度太小、不像 1.1 design」——CDP probe 證實實作正確、HMR 重載新值也正確生效，問題在 **spec 數值本身過於 subtle**。

經 4 輪 user-as-design-team 交互式調整（2px → 4px → 8px → ±50px mirrored dwell-return）後 user 鎖定數值組合：

- 鏡像：`scaleX(-1)`（公車朝左）
- X 軸：±50px range（總 100px traversal）
- Y 軸：-3px bumpy（與 X 同 keyframe）
- Rotation：±2°（-2° → 0° → +2° → 0° → -1° dwell）
- Loop：2.5s with dwell-return arc（forward → end dwell → return）

實作落在 `codebus-app/src/styles/globals.css` line 111-138（新 `codebus-bus-roll-mirrored` keyframe + `.codebus-bus-idle` selector 改指向新 keyframe），但 spec / AUDIT brand motion lock / LO-3 vocab / ODI-1 archived entry **未同步**。

### Pre-apply 校準（grep 證實後填）

按 [[feedback_propose_v1_spec_landing_read_audit_first]] + [[project_phase_3a_blind_spots_cleanup_lessons]] 教訓，propose 階段已 verbatim Read 5 source segment + grep 校準，結論如下：

| 校準項 | 預期 | 實測 | 結論 |
|---|---|---|---|
| `codebus-bus-idle-y` / `codebus-bus-idle-x` active consumer | 0 hit（archive 不算） | live code 只在 globals.css 定義、無 selector 引用、archive 文檔 2 hit | **可刪、本 change 同步處理** |
| `codebus-bus-roll-mirrored` 現況 | 已加 | globals.css line 122-129 已落、`.codebus-bus-idle` 已指向它 | 實作已 ship、本 change 只補 spec |
| `Idling in place` 出現點 | spec / AUDIT / globals.css comment | AUDIT × 2（line 548 + 561）、design-reply.html × 1（archive doc，不動）、globals.css comment × 1（line 113） | live 3 處須 obsolete |
| `EmptyState.test.tsx:63` 斷言 | class name only | `expect(heroes.length).toBe(1)` + `textContent === "🚌"`、未斷言 keyframe 名 / 數值 | **test 不需改** |
| `codebus-bus-roll`（LoadingOverlay）保護 | 不動 | `LoadingOverlay.tsx` 仍用 `codebus-bus-roll` 1.8s 單向 | 本 change scope 不含 |

### 同名詞 disambiguation

承 [[project_quiz_fullscreen_wizard_view_term_disambiguation]] 教訓。「mood」概念跨三段重寫、必須一次校齊避免新一輪 drift：

| 詞 | revise 前 | revise 後 |
|---|---|---|
| 「Idling in place」mood | 04b Lobby idle、subtle 2px/1px bob、無 rotation | **obsolete**、本 change 整個廢除此 mood |
| 「Moving forward」mood | LoadingOverlay 用、1.8s ±26px ±2° rotate | 重新定位（見下 Decision 1） |
| 04b Lobby empty hero motion 歸屬 | 「Idling in place」mood | 改歸「Moving forward · cyclic variant」（per Decision 1） |

## Goals / Non-Goals

**Goals:**

- Spec / AUDIT / code comment / archived design entry 對「04b Lobby empty hero motion」描述同步、消除實作領先 spec 的 drift
- Brand motion lock 概念表達更精準（命名 + 分類）、避免「idle」誤導未來 reader 以為是低能量微動
- Archive trail 完整：ODI-1 archived entry 加 revision footnote，保留決策歷史不重寫已封存內容

**Non-Goals:**

- 不動 LoadingOverlay 行為或其 `codebus-bus-roll` keyframe
- 不重啟 motion magnitude 調參（user 已 lock ±50px / 2.5s / mirrored / dwell-return）
- 不擴張 bus motion 到其他 surface（Goal Running / Quiz generation / Wordmark 既有 hard no 全保留）
- 不收 LO-2 / LO-4 等 LoadingOverlay 議題
- 不引入新 motion library / 不改 prefers-reduced-motion fallback 約束

## Decisions

### Decision 1 · Brand Motion Vocabulary 架構選定 Moving forward family + loading cyclic 變體

**Choice**: Option 2（family + variant）。AUDIT line 540-562 重寫為單一 `Moving forward` 家族，含兩個 variant：

| Variant | Keyframe | 用在 | 概念差異 |
|---|---|---|---|
| **loading**（單向 → 終點） | `codebus-bus-roll`（translateX -26→12 + ±2° rotate + Y bob, 1.8s） | LoadingOverlay（vault init） | 有起點、有終點、做完就停 |
| **cyclic**（mirrored 巡迴） | `codebus-bus-roll-mirrored`（translateX ±50 + ±2° rotate + Y -3px + dwell-return, 2.5s scaleX(-1)） | 04b Lobby empty hero | ambient 巡迴、無終點、user 沒動作就一直在 |

**Alternatives**:

- Option 1（3 平行 mood）：`Moving forward · loading` + `Moving forward · cyclic` + 廢除 `Idling in place` → reject，兩者共享「移動的 bus」核心隱喻，並列 3 個會稀釋 family 概念、未來新增 motion 還要再新增 mood。
- Status quo（保留 Idling in place mood）：直接 reject，user-as-design 已廢除。

**Why family + variant**: 兩 variant 共用「公車移動」核心語意（不是真 idle）、共享 prefers-reduced-motion 約束、共享 hard nos——family 結構更貼合實際相似性，未來若 02 Goal Detail 加 inline spinner（LO-3 提的 TBD）可以順理成章加 `inline variant`，不污染 mood 系統。

### Decision 2 · Spec Requirement 標題改 Lobby Empty State Hero Motion

原標題 `Lobby Empty State Idle Motion`，「idle」用詞已誤導——新 motion 是 cyclic ambient、不是 idle。改為 `Hero Motion` 中性、跟 04b empty-state hero 角色用詞對齊（spec 內既稱 "empty-state hero"）。

**Alternative**: 保留 `Idle Motion` 詞 → reject，跟 Decision 1 的 vocabulary 改革語意衝突。

### Decision 3 · 廢棄 keyframe codebus-bus-idle-y 與 codebus-bus-idle-x 直接刪除不留 fallback

grep 證實 live code 0 consumer（`.codebus-bus-idle` selector 已改指 `codebus-bus-roll-mirrored`）、僅 archive 文檔有歷史引用。刪除避免 dead code、降未來誤用風險。

**Alternative**: 留下 + 加 `/* deprecated */` 註解 → reject，per `coding-style.md` dead code 應刪除，且 archive doc 保留歷史快照已足。

### Decision 4 · ODI-1 Archived Entry 不重寫本體只 append Revision Footnote

ODI-1 已於 2026-05-27 archived（line 1827-1837），代表 design 階段的歷史決策——直接改寫等同竄改歷史。改採在 archived entry 末尾加 **Revision 2026-05-28** footnote、引用本 change 為新規格 source of truth。

**Alternative**: 重寫 entry 本體 + 改 archived 日期 → reject，破壞 archive trail 可追溯性。

### Decision 5 · globals.css comment 改成正式 spec 引用而非 TEMP experiment

現況 comment line 111-113 還寫 "TEMP experiment 2026-05-28" + 引用「Bumpy road / Idling in place」——本 change ship 後不再 temp、不再 idle。改寫成引用新 spec requirement 名 `Lobby Empty State Hero Motion`，讓 reader 可從 CSS 直接溯源至 spec。

## Implementation Contract

### Behavior（user-observable）

- 04b empty-state hero 🚌 emoji 渲染為 `scaleX(-1)` 鏡像（公車朝左）、執行 `codebus-bus-roll-mirrored` 2.5s loop：X 軸 -50px → +50px → dwell → return、Y 軸 -3px bumpy、rotation ±2°
- `prefers-reduced-motion: reduce` 開啟時 → `.codebus-bus-idle` `animation: none`、emoji 完全靜態
- LoadingOverlay 的 `codebus-bus-roll` 行為**完全不變**（單向、未鏡像、1.8s）
- Topbar wordmark 🚌 行為**完全不變**（靜態）

### Interface / data shape

- CSS selector：`.codebus-bus-idle`（class 名稱不變，動畫 target 改指新 keyframe）
- CSS keyframe 新增：`@keyframes codebus-bus-roll-mirrored`（已落地、本 change 不再改數值）
- CSS keyframe 移除：`@keyframes codebus-bus-idle-y`、`@keyframes codebus-bus-idle-x`
- Spec requirement key：`Lobby Empty State Hero Motion`（原 `Lobby Empty State Idle Motion`）
- AUDIT section anchor：`## Cross-cutting · Motion Vocabulary` 表格從 2 mood 改 1 family 2 variant

### Failure modes

- 若 `prefers-reduced-motion: reduce` fallback 失效 → spec violation、CDP smoke 步驟 3b 抓
- 若刪除 `codebus-bus-idle-y` 與 `codebus-bus-idle-x` 後有未察覺 consumer → `pnpm tsc` 不會抓（CSS 不參與 TS check）、但 `pnpm test` `EmptyState.test.tsx` 仍綠（只斷言 class）、實機 smoke 視覺會直接看到動畫斷掉
- 若 AUDIT「Idling in place」hard-coded 詞未一次清乾淨 → grep "Idling in place" 應 = 0 hit 於 live source（archive doc 不算）

### Acceptance criteria

1. `pnpm tsc` 在 codebus-app 綠
2. `pnpm test` 在 codebus-app 綠（重點：`EmptyState.test.tsx` "applies the idle-motion class on the empty-state hero only" 仍通過）
3. Grep `Idling in place` 於 `openspec/specs/` + `codebus-app/design-handoff/AUDIT.md` + `codebus-app/src/` → 0 hit
4. Grep `codebus-bus-idle-y` 與 `codebus-bus-idle-x` 於 `codebus-app/src/` → 0 hit
5. Grep `codebus-bus-roll-mirrored` 於 `codebus-app/src/styles/globals.css` → ≥1 hit（confirm 未誤刪）
6. Grep `codebus-bus-roll`（無 -mirrored 後綴）於 `codebus-app/src/styles/globals.css` + `codebus-app/src/components/LoadingOverlay.tsx` → ≥1 hit each（confirm LoadingOverlay 未被污染）
7. AUDIT line ~540-562 Motion Vocabulary 段落含 `Moving forward` family + `loading` + `cyclic` 兩 variant，無 `Idling in place`
8. AUDIT LO-3 段落（~line 625-630）vocab 更新、明列 `codebus-bus-roll`（loading）vs `codebus-bus-roll-mirrored`（cyclic）對應 surface
9. AUDIT ODI-1 archived entry（~line 1827-1837）末尾含 `Revision 2026-05-28 · lobby-hero-motion-revise` footnote、引用 archived change 路徑
10. `openspec/specs/app-shell/spec.md` `Requirement: Lobby Empty State Hero Motion` 存在、`Idle Motion` 同名 requirement 不存在
11. CDP smoke（per [[project_cdp_smoke_webview2_pitfalls]]、[[project_webview2_cdp_real_frontend]]）：
    - 開 dev server + WebView2 `--remote-debugging-port=9222`、`pnpm cdp` script connect
    - 路徑 a：開 Lobby 04b empty state（無 vault）→ 截圖至 `codebus-app/scripts/.lobby-hero-motion-revise-smoke/`、肉眼確認 🚌 朝左 + 動的範圍明顯（總 100px traversal）+ rotation 可見 + dwell-return 表達 ambient 巡迴（非真前進）
    - 路徑 b：透過 CSSOM 驗 `prefers-reduced-motion: reduce` rule 存在（CDP `Emulation.setEmulatedMedia` 不吃 WebView2、退一步驗）
    - 路徑 c：用 `__codebus_test_add_vault__` trigger LoadingOverlay → 截圖 confirm 公車仍朝右（`codebus-bus-roll` 未被污染）、1.8s loop

### Scope boundaries

**In scope:**

- `openspec/specs/app-shell/spec.md` `Lobby Empty State Idle Motion` requirement 完整重寫（含 rename + 正文 + 3 個 scenario 數值更新）
- `codebus-app/design-handoff/AUDIT.md` 三段：line ~540-562 brand motion lock、line ~625-630 LO-3 vocab、line ~1827-1837 ODI-1 archived footnote
- `codebus-app/src/styles/globals.css` line ~111-138：comment 重寫 + 刪除 `codebus-bus-idle-y` 與 `codebus-bus-idle-x` keyframe

**Out of scope:**

- LoadingOverlay 行為 / 樣式 / 文案
- Wordmark 🚌（topbar）
- Goal Running / Quiz generation 任何 bus motion
- `v1.1-mocks.html`、`design-reply.html`、`FEEDBACK.md` 等 design handoff doc（archive 階段不重寫，保留歷史快照）
- `EmptyState.tsx` component code（class name `.codebus-bus-idle` 不變、無須改）
- `EmptyState.test.tsx`（assertion 已 class-only、無須改；apply 階段再 grep 確認一次）
- motion 數值再調整（user 已 lock ±50px / 2.5s / mirrored / dwell-return）

## Risks / Trade-offs

- **Risk**: AUDIT 是長 doc（>1800 line），三段同時改易漏一處留下 stale 「Idling in place」 → **Mitigation**: acceptance criteria #3 grep 0 hit 強制收口、apply Task 1.1 重 grep 校準
- **Risk**: Brand motion lock 從「2 mood」改「1 family + 2 variant」是概念架構改動、未來新 motion 提案 mapping 規則跟著變 → **Mitigation**: design.md Decision 1 表格 + AUDIT 改寫後表格本身就是新 contract，未來提案直接對表
- **Risk**: 刪 `codebus-bus-idle-y` 與 `codebus-bus-idle-x` 後若有遺漏 selector 引用、視覺斷掉 → **Mitigation**: acceptance criteria #4 grep + #11 CDP smoke 視覺確認雙保險
- **Risk**: ODI-1 archived entry 加 footnote 可能與「不重寫 archived 本體」原則衝突 → **Trade-off**: footnote 屬 append-only 修訂史、不改變 archived 決策本體；明確標 `Revision 2026-05-28` 與引用本 change name，可追溯性優於沉默 drift
- **Trade-off**: 不在 apply 階段順手改 `codebus-bus-roll`（LoadingOverlay）統一 keyframe 命名（如 `codebus-bus-roll-forward`）→ 故意不做，per Non-Goals。命名一致性不值得擴大 scope；若未來真要做、開獨立 change。
