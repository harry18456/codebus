# Backlog

未來 TODO 集中表。每條指向 `docs/<date>-<slug>-backlog.md` 看完整描述、proposed fix、工程量。新發現的 design smell / UX 缺陷 / feature gap 都記在這——之後再決定要不要 `/spectra-propose` 起 change。

## 開放項目

| 加入日期 | 標題 | 嚴重度 | 工程量 | 詳細文件 |
|---|---|---|---|---|
| 2026-05-14 | 全域 font-scale / accessibility text size | accessibility gap | 中（2-3 個半天） | [app-font-scale](2026-05-14-app-font-scale-backlog.md) |
| 2026-05-14 | UI 無障礙（對比度 + 鍵盤導航） | accessibility gap | 中（2-3 個半天） | [ui-accessibility](2026-05-14-ui-accessibility-backlog.md) |
| 2026-05-14 | multi-provider agent backend（Codex CLI + Azure） | 架構擴充性 | 重（1 週以上） | [multi-provider-agent-backend](2026-05-14-multi-provider-agent-backend-backlog.md) |
| 2026-05-14 | OpenAI Privacy Filter 整合（local 語意層 PII） | PII 保護強化 | 重（3-5 個半天） | [openai-privacy-filter](2026-05-14-openai-privacy-filter-backlog.md) |
| 2026-05-14 | RAG index + search（LanceDB，after F） | 知識檢索品質 | 重（1 週以上） | [rag-index-search](2026-05-14-rag-index-search-backlog.md) |
| 2026-05-14 | codebus 作為 MCP Server（query-only，after F） | 擴充性 / 生態整合 | 中-重（3-5 個半天） | [mcp-server](2026-05-14-mcp-server-backlog.md) |
| 2026-05-14 | MyCoder CLI 整合 | pending（等對方 CLI 長出 contract，見 2026-05-20 spike 結論） | 中（spike 後定） | [mycoder-cli](2026-05-14-mycoder-cli-backlog.md) |
| 2026-05-14 | GitHub 倉庫設定（Actions CI + Release + Issue templates） | release readiness | 輕-中（1-2 個半天） | [github-repo-setup](2026-05-14-github-repo-setup-backlog.md) |
| 2026-05-14 | Settings 缺少 chat verb 的 model / effort 設定 | UX gap（設定不透明） | 輕-中（方案 A 半天 / 方案 B 1-2 半天） | [settings-chat-model](2026-05-14-settings-chat-model-backlog.md) |
| 2026-05-19 | Settings 設定面板完整化（config↔UI 覆蓋盤點） | UX gap（設定不透明） | Change 1 ✅ ship; Change 2 todo | [settings-config-coverage](2026-05-19-settings-config-coverage-backlog.md) |
| 2026-05-20 | Wiki 頁面加按鈕直接開 Obsidian | UX 補強（取代 in-app graph view） | 輕（半天） | [wiki-open-in-obsidian](2026-05-20-wiki-open-in-obsidian-backlog.md) |

## 已 archived 項目

| Archive 日期 | 標題 | 對應 change | 詳細文件 |
|---|---|---|---|
| 2026-05-14 | skill bundles repo-root copy 改 opt-in | `v3-skill-bundles-vault-only` | [skill-bundles-vault-only](2026-05-14-skill-bundles-vault-only-backlog.md) |
| 2026-05-20 | PII 設定 UI（Settings 內加 extra regex rules） | `settings-config-frontend` | [pii-settings-ui](2026-05-14-pii-settings-ui-backlog.md) |
| 2026-05-20 | .codebus 目錄即時監聽（fs watcher） | `codebus-fs-watcher` | [codebus-fs-watcher](2026-05-15-codebus-fs-watcher-backlog.md) |
| 2026-05-20 | raw mirror 巢狀 .git 未排除（submodule leak） | `raw-sync-nested-git-leak` | [raw-sync-nested-git-leak](2026-05-19-raw-sync-nested-git-leak-backlog.md) |
| 2026-05-20 | PreToolUse Read hook 擋圖片 / binary 檔案 | `pretooluse-image-block` | [pretooluse-image-block](2026-05-20-pretooluse-image-block-backlog.md) |

## 已決定不做

無對應 change，但留 backlog 文件以保決策脈絡（之後再翻出來不會以為「沒人想過」）。

| 結案日期 | 標題 | 理由 | 詳細文件 |
|---|---|---|---|
| 2026-05-20 | PII-aware git context tool | 替代「什麼都不做」可接受：source code 已 mirror 進 raw/，wiki 不缺；`raw-sync-nested-git-leak` 已把「不複製 .git」安全 floor 收掉 | [git-context-tool](2026-05-14-git-context-tool-backlog.md) |
| 2026-05-20 | Wiki 網路圖（Obsidian-style graph view） | 改用「按鈕直接開 Obsidian」取代當下需求；in-app graph 等 v2 真有沒裝 Obsidian 的使用者再開（見 [wiki-open-in-obsidian](2026-05-20-wiki-open-in-obsidian-backlog.md)） | [wiki-graph-view](2026-05-20-wiki-graph-view-backlog.md) |

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
