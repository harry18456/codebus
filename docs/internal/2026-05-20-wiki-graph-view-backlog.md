# Backlog: Wiki 網路圖（Obsidian-style graph view）

**Date:** 2026-05-20
**Surfaced during:** session 收尾要求
**Severity:** feature gap（v1 明確 out-of-scope，user 反映想要）
**Owner:** harry
**Status:** parked

---

## 觀察

Obsidian 最具辨識度的功能之一是 graph view —— 把 vault 內所有 page 當節點、
`[[wikilink]]` 當邊，render 成可縮放/拖曳的網路圖。codebus-app v1 在
`openspec/specs/app-shell/spec.md` 的「Forbidden Behaviors in v1」requirement
裡明文列為禁止項：「Graph view entry in any sidebar」。本條 backlog 是「v1
之後值得做」的紀錄。

底層資料**已經可以推**：
- `.codebus/wiki/**/*.md` 是 page 集合
- `codebus-core/src/wiki/lint/` 的 `broken_wikilink` rule 已經會 parse 出
  `[[wikilink]]` 目標 → 邊的來源已存在
- frontmatter 已含 taxonomy folder（5 類）→ 節點可上色分類

缺的是：
- 一個聚合「page → outbound links + inbound backlinks」的 Tauri command
- 前端 graph rendering 元件

## Proposed fix

### 資料層

新增 IPC `list_wiki_graph(vault_path) -> { nodes: [{slug, title, folder}], edges: [{from, to}] }`：
- 復用 lint 既有的 wikilink parser 一次掃完整 vault
- broken wikilinks 不進 edges（或標 `broken: true` 讓前端決定顯示）

### 渲染層（三個務實選項）

| 選項 | Lib | 重量 | 互動 |
|---|---|---|---|
| A | [Cytoscape.js](https://js.cytoscape.org/) | 中 | 拖曳 / zoom / 點擊跳 wiki / 過濾 / force-directed layout 內建 |
| B | [react-force-graph](https://github.com/vasturiano/react-force-graph) | 輕 | API 簡、Canvas 渲染快、UX 接近 Obsidian |
| C | [Sigma.js](https://www.sigmajs.org/) | 中-重 | 大型圖效能好（>1k 節點） |

預期 vault 規模 < 數百 page 時 **B 最快上線**；若未來 RAG / 大型 vault 再考慮 C。

### UX

- Workspace 加 Graph tab（與 Goals / Wiki / Quiz 並列）
- 節點顏色 = taxonomy folder（concepts / entities / modules / processes / synthesis）
- 點節點 → 開該 wiki page 預覽（重用既有 Milkdown preview）
- hover 節點 → highlight 鄰居 + dim 其他
- 搜尋框：filter by slug / title 子字串

### Config（可選）

- `app.graph.layout`: `force` / `radial` / `grid`（v1 只做 force）
- `app.graph.show_orphans`: bool（無 link 的 page 是否顯示）

## Tasks 粗估

1. `codebus-core` 新增 `wiki::graph` 模組 export `WikiGraph { nodes, edges }`，內部復用 lint 既有 parser
2. Tauri IPC `list_wiki_graph` + 單元測試
3. 前端 store + IPC 串接 + 一個 GraphTab 元件（react-force-graph）
4. Workspace tab 加 Graph 條目（i18n）
5. 互動：點節點跳 wiki preview、hover highlight、broken link 視覺
6. 測試：圖資料 round-trip、broken link 處理、空 vault edge case

工程量：中（3-4 個半天，含挑 lib + UX）。

## Out of scope

- backlinks panel（雖然底層資料一樣，獨立 UX，另記）
- graph 即時更新（用 polling refresh 或等 `codebus-fs-watcher` backlog）
- 3D 視圖
- 把 raw code 檔當節點（只 wiki page、不混源碼）

## 何時動

明確列為 **v1 之後**（v1 Forbidden Behaviors 限定）。動 F `v3-app-polish-ship`
完、v3.x release 後再評估；或當 user 反映「沒網路圖找不到 page」時提前。
與 `codebus-fs-watcher`（即時感知外部變動）併做最自然。
