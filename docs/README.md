# CodeBus 文件導覽

> 本資料夾是 CodeBus 全部設計文件。Review 時建議照「閱讀順序」走，新加入成員照「onboarding 順序」走。

---

## 一、文件分類

### A. 產品 / 總體
| 文件 | 一句話 |
|---|---|
| [`../README.md`](../README.md) | 產品定位、差異化、功能規劃、Module 清單、常見尖銳問答 |

### B. 決策與脈絡
| 文件 | 一句話 |
|---|---|
| [`decisions.md`](decisions.md) | ADR 風格決策日誌（D-001 ~ D-020），每決策含脈絡 / 選項 / 理由 / 後續 |

### C. 架構橫切 spec
| 文件 | 一句話 |
|---|---|
| [`security.md`](security.md) | 合規對映表、資安設計原則、Demo checklist |
| [`sanitizer.md`](sanitizer.md) | PII / Secret / 內部識別符去識別化（D-015） |
| [`tool-sandbox.md`](tool-sandbox.md) | Agent 工具執行邊界、路徑白名單、Sandbox helper（D-017） |
| [`llm-provider.md`](llm-provider.md) | LLM Provider 抽象 Protocol（D-003） |
| [`sidecar-api.md`](sidecar-api.md) | Python Sidecar HTTP API + SSE 事件 schema |
| [`authorization.md`](authorization.md) | O-01 授權 Modal spec + `authorization_audit.jsonl` 事件（D-008） |
| [`workspace-lifecycle.md`](workspace-lifecycle.md) | Workspace 資料分級 / R-00 Start Page / 遺失恢復策略（D-023 / D-024 / D-025） |

### D. Agent 設計
| 文件 | 一句話 |
|---|---|
| [`agent-explorer-spec.md`](agent-explorer-spec.md) | Explorer Agent 決策邏輯、三層架構、評估 |
| [`agent-core.md`](agent-core.md) | ReAct loop 實作 spec（自寫 + Instructor，D-012） |
| [`qa-agent.md`](qa-agent.md) | Q&A Agent + KB 自動成長（D-016） |
| [`prompts.md`](prompts.md) | 五份 Agent system prompt 骨架 |

### E. Module 細節
| 文件 | 一句話 |
|---|---|
| [`module-1-scanner.md`](module-1-scanner.md) | Folder Scanner：遍歷 / gitignore / binary / encoding / git metadata |
| [`module-2-kb-builder.md`](module-2-kb-builder.md) | KB Builder：Qdrant schema / chunk / embedding / 去重 |
| [`module-5-generator.md`](module-5-generator.md) | Markdown Generator：prompt 結構 / 元件驗證 / plain mode |
| [`interactive-tutorial.md`](interactive-tutorial.md) | 前端互動（投影片 + Checkpoint + Quiz）Module 5 / 7 介面契約 |

### F. 開發與測試
| 文件 | 一句話 |
|---|---|
| [`dev-setup.md`](dev-setup.md) | 從 clone 到跑起來 onboarding 指南 |
| [`implementation-plan.md`](implementation-plan.md) | 跨模組依賴 + 30 步實作順序 + 里程碑檢核點（縫合各 module 實作順序章節） |
| [`../tests/golden/timeline-gdrive-adapter/ideal-route.md`](../tests/golden/timeline-gdrive-adapter/ideal-route.md) | Demo task 的 ideal route（D-004 / D-006） |

---

## 二、Review 閱讀順序（建議）

完整脈絡看一輪：

1. **`../README.md`** — 產品是什麼、要做什麼
2. **`decisions.md`** — 一路走過什麼決定、為什麼（掃 summary table 即可）
3. **Agent 設計系列**（核心競爭力）：
   - `agent-explorer-spec.md` — 探索策略
   - `agent-core.md` — ReAct loop 實作選型
   - `qa-agent.md` — 教材後 Q&A
   - `prompts.md` — 五份 prompt 骨架
4. **橫切 spec**：
   - `security.md` → `sanitizer.md` → `tool-sandbox.md`（兩層防線）
   - `llm-provider.md` → `sidecar-api.md`（系統邊界）
5. **Module 細節**：
   - `module-1-scanner.md` → `module-2-kb-builder.md` → `module-5-generator.md`
   - `interactive-tutorial.md` 再對照 Module 5
6. **Golden sample**：`tests/golden/timeline-gdrive-adapter/ideal-route.md`
7. **Dev**：`dev-setup.md`

---

## 三、Onboarding 順序（給未來加入的人）

1. `../README.md`（知道我們在做什麼）
2. `dev-setup.md`（跑起來）
3. 挑一個 Module 的 spec 深入（依 PR 範圍）
4. 遇到合規 / Sandbox / Sanitizer 細節再回來 C 組

---

## 四、文件完成度

| 狀態 | 文件 | 說明 |
|---|---|---|
| ✅ 完稿 | 15 份主 spec | 上方清單全部 |
| 🟡 待使用者 review | `sanitizer.md` / `module-5-generator.md` / `prompts.md` / `dev-setup.md` | 含 `TODO review` 項 |
| 🟡 待 golden sample 調整 | `prompts.md` 五份骨架 | 實作期對照分數迭代 |
| 🟡 待實作回頭補 | `module-2-kb-builder.md` embedding dim、`dev-setup.md` .env 模型名 | LLM 供應商 API 細節確認後填 |
| ❌ MVP 範圍外 | `Module 3 Topic Explorer` 相關 | Phase 2（`agent-explorer-spec.md` §十二 有預留設計） |
| ❌ 待補 | Module 6 介入控制器 | 前端實作時決定，現在 spec 會過早（D-020，詳見 `decisions.md`） |

---

## 五、重要 cross-reference

| 主題 | 相關文件 |
|---|---|
| 資料送 LLM 前的保護 | `sanitizer.md` + `llm-provider.md` §六 |
| Agent 工具邊界 | `tool-sandbox.md` + `agent-core.md` §六 ToolContext |
| KB 如何成長 | `qa-agent.md` §三 + `module-2-kb-builder.md` §八 |
| Explorer / Q&A 共用 | `agent-explorer-spec.md` §十二 trait + `agent-core.md` §二 + `qa-agent.md` §二 |
| 教材互動 | `interactive-tutorial.md` + `module-5-generator.md` §四 |
| 合規對映 | `security.md` + `tool-sandbox.md` §十二 + `sanitizer.md` §七 |
| Golden sample 機制 | `decisions.md` D-006 + `tests/golden/*/ideal-route.md` |
| Workspace 生命週期 | `workspace-lifecycle.md` + `authorization.md` + `decisions.md` D-023 / D-024 / D-025 |
