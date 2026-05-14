# Backlog

未來 TODO 集中表。每條指向 `docs/<date>-<slug>-backlog.md` 看完整描述、proposed fix、工程量。新發現的 design smell / UX 缺陷 / feature gap 都記在這——之後再決定要不要 `/spectra-propose` 起 change。

## 開放項目

| 加入日期 | 標題 | 嚴重度 | 工程量 | 詳細文件 |
|---|---|---|---|---|
| 2026-05-14 | skill bundles repo-root copy 改 opt-in | design smell | 小（1-2 task） | [skill-bundles-vault-only](2026-05-14-skill-bundles-vault-only-backlog.md) |
| 2026-05-14 | 加 PII-aware git context tool 給 agent | feature gap + PII safety | 中（2-3 個半天） | [git-context-tool](2026-05-14-git-context-tool-backlog.md) |

## 已 archived 項目

（移到此區當對應 change 已 archive、不再開放）

---

## 怎麼加新項目

1. 在 `docs/` 建 `YYYY-MM-DD-<slug>-backlog.md`，內容仿照既有兩條格式：
   - 觀察 / 問題描述
   - Proposed fix（如有多方案列出）
   - Tasks 粗估 + 工程量
   - Out of scope
   - 何時動 / 優先序
2. 在這份 `BACKLOG.md` 的「開放項目」表加一列
3. 之後若決定動，用 `/spectra-propose <slug>` 把該 backlog 當 pre-discuss 帶進 propose flow

## 怎麼歸檔

對應 change archive 後（`spectra archive <change-name>`），把這項從「開放項目」移到「已 archived 項目」並標明對應 change 名稱 + archive 日期。
