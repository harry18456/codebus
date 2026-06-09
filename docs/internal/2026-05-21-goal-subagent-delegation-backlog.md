# Backlog: 在 goal 引入動態 subagent 委派（Task 工具）

**Date:** 2026-05-21
**Surfaced during:** discuss 2026-05-21（subagent 控制/使用性實測後延伸）
**Severity:** capability enhancement（agentic 自主性）
**Owner:** harry
**Status:** open — 先記錄，未來再評估要不要做

---

## 動機

codebus 目前每個 verb 都是「單一受限 agent」（toolset 不含 `Task`，無法開 subagent）。討論結論：agentic AI 的價值有一部分就是「有工具可自主選用」，給它委派能力（讓 AI 自行判斷要不要 spin off 子調查、隔離 context）在 agentic 設計上合理。四個 verb 評估後，**只有 goal 適合**——它是唯一會橫跨多模組大量讀檔 + 合成 + 寫 wiki 的 verb，正是 subagent「context 隔離 + 平行探索」能發揮的地方。query/chat（唯讀輕快問答）、fix（窄修補）適配度低。

## 兩個可分開的決定

- **(A) 給「委派能力」**：把 `Task` 加進 `GOAL_TOOLSET` + goal SKILL.md 輕引導（大主題才委派）。幾乎零成本、可逆、可隨時拔。這是「該給 AI 工具」論點真正支持的部分。
- **(B) 設計專職 researcher 契約**：vault 內 ship 一個唯讀 researcher agent def（結構化回傳：模組職責 / 關鍵檔 / 對外介面），主 agent 仍負責寫所有 wiki。較高設計投入、更可控。

## Grounding（已確認）

- **spawn cwd = `.codebus/`**（`init.rs:90`、`skill_bundle/mod.rs:4`）→ project-scope agent def 應 ship 到 `.codebus/.claude/agents/researcher.md`（vault-internal，跟 skill bundle 同授權路徑），**不污染 repo root**。
- **安全已驗證**（見 [subagent-sandbox-control](2026-05-21-subagent-sandbox-control-backlog.md)）：subagent 受 parent `--tools` 天花板約束、`--strict-mcp-config` 自動套用、ambient MCP 不下放。唯讀 researcher（`tools: Read,Glob,Grep`）即使主 agent 有 Write 也拿不到 Write。

## 關鍵坑（未驗證，動工前必測）

goal 的 parent toolset 含 **Write/Edit**。只給 Task 不引導時，AI 預設叫內建 `general-purpose` subagent（實測過 fallback 行為）。**推論** general-purpose 無 frontmatter 工具限制 → 可能繼承 parent 全套（含 Write/Edit）→ 等於「開放 subagent 也能寫 wiki」，跟「主 agent 寫、researcher 唯讀探索」的乾淨模型衝突（多 agent 不協調寫檔、變更難歸因）。

**未驗證**：general-purpose subagent 在 goal toolset（含 Write）下到底能不能實際寫檔。Test A 測的是「自訂 def 要 Write 但 parent 沒給 → 擋住」；general-purpose「無 def 限制 + parent 有 Write」是另一種情況，需另測（檔案系統 ground truth）。

## 若要做，建議路徑

1. **先補 ground-truth 測**：goal toolset 下 general-purpose subagent 能否寫檔（決定 (A) 是否裸給就會開放 subagent 寫）。
2. 若能寫 → (A)+(B) 一起：給 Task 同時 ship 唯讀 researcher def + prompt 明確引導「探索委派給 researcher，別用 general-purpose」，把委派 channel 進唯讀路徑。B 取最小版（唯讀 + 簡單回傳格式）。
3. **第一版當可量測實驗**：大 repo 跑幾次 `goal --debug`，看 AI 會不會用、用了 wiki 品質變好還是變亂，再決定要不要把 researcher 契約做厚。

## 為何先不做（deferral 理由）

- **無實測痛點**：目前沒有證據顯示 goal 在大 repo 撞 context 上限或讀檔失控；屬最佳化非結構必要（deletion test：刪掉 researcher，goal 照樣 work）。
- **低成本、可逆**：(A) 是 toolset 加一條 + prompt 一句，隨時可加可拔——延後成本極低。
- 待出現「goal 在大 repo 品質/context 有感問題」或「想主動實驗 agentic 委派」的訊號再動。

## Out of scope

- query / chat / fix 加 Task（適配度低，不在本條）
- 把現有 content-verify reviewer 改成 subagent（討論結論：維持獨立 spawn 較適合「必跑 + 獨立 model + 結構化輸出 + repair loop」，見該次討論）
- 平行多 subagent 編排 / subagent 間通訊（過早）

## 何時動

無硬依賴。優先序低於有實測痛點的項目；屬「想實驗 agentic 自主委派」時的入口。動工前先做上述 general-purpose 寫檔 ground-truth 測，再起獨立 change。
