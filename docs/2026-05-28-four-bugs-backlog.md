# 四個 bug backlog（2026-05-28）

`lobby-hero-motion-revise` archive 後依序處理。

## Bug 1 · 「搞懂這 repo」missing i18n

**現象**：UI 某處顯示「搞懂這 repo」字面、應該走 i18n bundle 但 hard-code 中文。

**狀態**：site 位置未指、apply 第一步 grep 校準。

**Pre-apply 起手**：
```bash
grep -rn "搞懂這 repo\|搞懂這" codebus-app/src/
```
→ 找到 site → 確認 i18n key 是否已存在（reuse 既有 / 新增）→ wire。

**Severity**：低（cosmetic、en locale 中英混雜）

**屬性**：Phase 3A blind spot residual、跟 `phase-3a-blind-spots-cleanup` 系列同性質
（前面已踩 Pattern 1a / 1c / 6、這條可能 Pattern 1 漏抓 OR 新 site 在 Phase 3A 後加入）

---

## Bug 2 · ChatWidget bubble 小橘點觸發條件錯

**現象**：ChatWidget bubble 在「chat 回應時」就顯示橘點、user 認為應該是「設計上 chat 回應完成才有」。

**Spec 對照**（Phase 5.1 `chatwidget-pulse-and-cancel-move`）：
- ODI-4 spec：pulse dot = **active goal running** ambient signal、跟 chat session 無關
- 不是「chat 回應完成」、是「goal 在跑」

**可能成因**（猜、待 verify）：
- a. user 預期錯（pulse dot 本意就是 goal running、跟 chat 無關）—— 純文件 / 教學問題
- b. 實作 wire 錯：pulse dot 接到 chat in-progress state、不是 goal running state
- c. user 看到的「橘點」其實是別的 visual（如 token usage indicator？streaming caret？）—— misidentified

**Pre-apply 起手**：
1. CDP smoke 重現：開 chat、發訊息、看 bubble 上的橘點時機
2. Grep `pulse-dot` / `useActiveGoal` / chat session in-progress state 看 wire
3. 對照 Phase 5.1 archive design.md 看 spec 原意

**Severity**：中（signal correctness、user 看了會誤判 codebus 狀態）

---

## Bug 3 · Goal 跑完 → 切 vault → 新 vault 無法觸發 goal

**現象**：在 vault A 跑 goal → 回 Lobby → 進 vault B → 在 vault B 起新 goal 失敗。

**可能成因**（猜、待 verify）：
- a. **「One Active Goal Run At A Time」guard 沒釋放**：spec（`app-workspace` requirement「One Active Goal Run At A Time」）有此約束、可能 cross-vault state 沒 reset → vault B 認為 vault A 的 goal 還在跑
- b. **process 沒處理好**（user 描述用詞）：goal 後端 child process exit 後 frontend state 沒 clean、新 goal 觸發被擋
- c. **store reset on vault switch**：切 vault 時 useGoalsStore / 同類 store 沒清 active state

**Pre-apply 起手**：
1. CDP smoke 重現完整序列（vault A 跑 → 回 Lobby → vault B 進 → 起 goal）
2. Grep `oneActiveGoal` / `activeGoalRun` / vault switch handler 看 state lifecycle
3. Read spec `app-workspace` `One Active Goal Run At A Time` requirement 確認 spec 原意
4. 若是 a/c → frontend store 修；若是 b → 可能要動 backend goal process lifecycle

**Severity**：**高**（workflow blocker、user 切 vault 後完全卡死）

---

## Bug 4 · Codex 沒關 hosted web search

> **Status：archived 2026-05-28** — fix landed in
> `openspec/changes/archive/2026-05-28-backend-cleanup-codex-websearch-and-runid-millis/`.
> Real-CDP smoke 確認 codex 回 `web_search is not available in the current
> tool surface`、`isolation_flags_always_present` test 守住 regression。
> Image generation 維持保留（user 決議）。

**現象**：codebus 透過 codex provider 跑 verb 時、codex 默默上網查（hosted web search 仍 active）。

**Source**：`docs/2026-05-28-codex-hook-hard-gate-spike.md` 第 295+ 行 E11 spike report
（commit `7276b15`、本 session 早期 user 提的「working tree 有不屬 design audit 的 spike doc」就是這份）

**Spike 結論**（line 328-331）：
- 當前 codebus spawn flag `--disable apps` 移除 app/plugin tools、**不關** Codex provider-hosted web search
- 加 `-c web_search=disabled` 可關 hosted web search
- codebus 既有 isolation 配方：`--ignore-user-config` + `--disable apps` + `--ignore-rules` + `project_root_markers` + `-s` 需擴充加 `-c web_search=disabled`

**Pre-apply 起手**：
1. Grep `--disable apps` / `web_search` 在 codebus-core spawn config（看 codex provider integration 位置）
2. Read spike doc E11 verbatim 確認 flag 名 + 行為
3. 加 `-c web_search=disabled` 進 codex spawn args
4. 加 regression test（spike 提及）：確認新 codebus session web search 真的 unavailable

**Scope 限定（2026-05-28 user 決議）**：
- ✅ **關** hosted web search（加 `-c web_search=disabled`）
- ❌ **不關** hosted image generation（user 想保留、未來可能給 wiki diagram / visual ref 等場景用）
- spike doc E12 提到的 `--disable image_generation` flag **本 bug 不加**
- 若未來 image generation 變問題、再開另一個小 change 加 flag

**Severity**：中-高（privacy + isolation 假設 broken；codebus 宣傳「local code agent」但 codex 路徑悄悄出網查 web）

**屬性**：multi-provider F1-F4 backlog 範疇、屬 codex provider 整合補洞

---

## 建議處理順序

**3 → 4 → 2 → 1**（severity + scope risk）

| # | 為什麼這位置 |
|---|---|
| **3** | 最高（workflow blocker、user 切 vault 卡死）+ 可能要動 backend lifecycle |
| **4** | 中-高（privacy / isolation 違反）+ scope 已 spike 明確、加 flag 簡單 |
| **2** | 中（signal 正確性、可能只是文件 / 認知差） |
| **1** | 低（cosmetic） |

每個 bug 一個 spectra change，OR 合 1 個 `2026-05-28-four-bugs-cleanup`（看 scope 重疊度、apply 第一步決定；推測 3/4 各自獨立、1/2 可合）。

**Claude-trace prompt 分析**（`docs/2026-05-28-claude-trace-prompt-analysis-todo.md`、priority 中-高）：彈性插入、不強排序、user 2026-05-28 決定可後置。理想在 bug 3 archive 後做、但全 bug 收完再做也 acceptable。

---

## 觸發時機

`lobby-hero-motion-revise` archive + AUDIT update 完成後接續。
