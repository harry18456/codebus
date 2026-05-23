# Loop Worklog

Append-only。每輪一筆，最新在最上面。格式：

```
## YYYY-MM-DD HH:MM — T# <任務名>
- 狀態: DONE | BLOCKED
- 做了: <一兩句>
- 產出: docs/...
- 下一步 / BLOCKED 原因: ...
```

---

## 2026-05-23 00:5xZ — T9-T13 連跑收尾（harry 線上陪跑）
- 狀態: 5 個 DONE 一輪打完
- 做了:
  - **T9 spec drift**: 找到 3 真 drift（D1 lint-feedback-loop summary 沒提 quiz validate / D2 skill-bundles 寫「three」實際 five / D5 hook 無 metachar req）+ 2 成熟度缺口（D3 pii spec 未要求 disjoint→F1 源頭 / D4 skill-bundles 忠實描述失準實作）。
  - **T10 docs freshness**: R1 README 漏 `codebus config` 子命令、R2 整份 README 對 codex/multi-provider 完全沉默（含 sandbox 描述、CLI 需求）；R3-R5 次要。
  - **T11 mcp-server**: backlog 設計完整，補 incremental MVP 路線（現在可動三件唯讀 wiki 工具，不必全卡 F）。
  - **T12 rag-index-search**: backlog 完整，補三點（ONNX runtime 跨 T13 共用、注入路徑要 provider-neutral 避免重製 PE1 失準、先做 standalone search）。
  - **T13 openai-privacy-filter**: 3 更新（pii-settings-ui 已 archive 同批失效、**F1 必須先修**（語意+regex 重疊更頻繁）、ONNX runtime 與 T12 對齊）。
- 產出: 5 個 docs/2026-05-22-*.md
- 下一步: **佇列清空**——所有 13 個 PLAN 任務完成（PE1/PE2 + T1-T13）。若 harry 想繼續，可進「自我再規劃協定（RP）」（解除「只讀」邊界）；或選定實作項起 change。

## 2026-05-22 20:0xZ — T8 品質檢查 app（前端）
- 狀態: DONE
- 做了: 安全 lens 掃前端。無 dangerouslySetInnerHTML/any/ts-ignore，chat 連結渲染對 XSS 安全（external 僅 ^https?: + openExternalUrl，其餘惰性），wikilink encodeURIComponent + hasOwnProperty 防原型污染，QuizTab exhaustive-deps 抑制是正當 latch pattern。無真實 bug。實質產出：修正 T3 app 實作假設——transformBodyWikilinks 產 codebus:// href 會被 react-markdown 預設 urlTransform 洗掉、且現有 WIKI_HREF_RE 匹配不到該 scheme，T3 落地需多兩步（已回寫 T3 理解）。次要：EXTERNAL_HREF_RE 偏鬆、一個 i18n TODO。
- 產出: docs/2026-05-22-app-quality-review.md
- 下一步: 佇列下一個 TODO 是 T9（spec drift 檢查）。三個 crate 品質檢查（T6/T7/T8）完成：core/cli 各一安全 latent bug、app 乾淨。

## 2026-05-22 19:43Z — T7 品質檢查 cli
- 狀態: DONE
- 做了: 精讀 hook.rs（PreToolUse Bash/Read 閘）。找到高優先安全項 F4：is_allowed_bash_command 用 split_whitespace 只查 argv[0]=codebus + argv[1]=lint/quiz validate，不檢查其後 → shell 串接（codebus lint; rm -rf / 等）通過前綴檢查。command 會丟 sh -c 執行 → 可能 sandbox 逃逸。待驗證 Claude Code 串接是否拆段問 hook；不論如何建議 defense-in-depth 拒 shell 元字元。測試完全沒覆蓋串接案例。Read hook（image block）設計紮實。已進 BACKLOG。
- 產出: docs/2026-05-22-cli-quality-review.md（+ BACKLOG F4 列）
- 下一步: 佇列下一個 TODO 是 T8（codebus-app 前端品質檢查）。

## 2026-05-22 19:33Z — T6 品質檢查 core (Part 1: pii + git)
- 狀態: DONE
- 做了: 深讀 PII redaction 路徑（pii/ + raw_sync.rs）+ git/。找到真實 latent bug F1：mask_matches 註解假設 match 非重疊，但 scan() 從不合併跨 rule 重疊/巢狀 match → 倒序 replace_range 對重疊不安全，可能漏遮 secret 或輸出損壞（custom pattern 框 builtin 最易觸發）。F2：>5MiB 檔靜默排除無 warn。F3：changed_paths_under 把 deleted page 也算 changed。F1 已加進 BACKLOG（輕，interval-merge 約半天）。core 零 TODO/FIXME、安全 floor 設計正確。
- 產出: docs/2026-05-22-core-quality-review.md（+ BACKLOG F1 列）
- 下一步: 佇列下一個 TODO 是 T7（codebus-cli 品質檢查）。core 其餘大模組（verb/config/vault/wiki/log/render）列為後續 review 候選。

## 2026-05-22 19:4xZ — T5 spike goal-subagent-delegation
- 狀態: DONE
- 做了: 核對 grounding（GOAL_TOOLSET=Read/Glob/Grep/Write/Edit 無 Task、core 無 Task 引用、無 .claude/agents ship）皆屬實。關鍵阻塞（general-purpose 能否寫檔）需真實 claude run，loop 做不到。新缺口：整套 Task+--tools 機制 claude-only，codex 有內建 spawn_agent 不受 --tools 天花板約束、subagent-sandbox-control 安全驗證不涵蓋 codex → 此條變 provider-specific 兩套機制。建議維持 deferral + 更新原 backlog 補 provider 維度。
- 產出: docs/2026-05-22-goal-subagent-delegation-spike.md
- 下一步: 佇列下一個 TODO 是 T6（codebus-core 品質檢查 — 第一個非 backlog 的淨分析任務）。

## 2026-05-22 19:13Z — T4 spike github-repo-setup
- 狀態: DONE
- 做了: 核對 2026-05-14 backlog（過時最多的一條）。發現 3 drift：(1) 套件管理器是 npm 非 pnpm（package-lock.json）；(2) release 依賴的 F(v3-app-polish-ship) 未 archive 且 tauri.conf bundle.active=false → release workflow 無法寫；(3) workspace 含 src-tauri，cargo test --workspace 在 Linux 需 webkit2gtk 系統依賴。草擬了修正後的 CI workflow（core/cli 三平台 + app npm vitest/tsc + tauri-build-check 含 apt deps）+ issue/PR templates，皆可立即落地。release workflow 標 BLOCKED 待 F。
- 產出: docs/2026-05-22-github-repo-setup-spike.md
- 下一步: 佇列下一個 TODO 是 T5（goal-subagent-delegation spike）。

## 2026-05-22 19:03Z — T3 spike chat-display-polish
- 狀態: DONE
- 做了: 核對 2026-05-21 backlog。app 端 AssistantMarkdownBlock 缺 remarkGfm（WikiPreview 有、dep 已在）、WIKI_HREF_RE 只路由 markdown link 不處理 [[slug]]、milkdown-wikilink 的 transformBodyWikilinks/WikilinkLink 現成可重用；CLI chat.rs:192 raw println 無 markdown。皆屬實。標註：本條 provider-agnostic、無 PE2 耦合（operate on 正規化 Thought，兩 provider 一致）→ 可獨立先行；CLI [[slug]] 連結化已切給 cli-wikilink-link-target。
- 產出: docs/2026-05-22-chat-display-polish-spike.md
- 下一步: 佇列下一個 TODO 是 T4（github-repo-setup spike）。

## 2026-05-22 19:1xZ — T2 spike app-stream-verbose-detail
- 狀態: DONE
- 做了: 對現碼核對 2026-05-21 backlog（設計已收斂）：截斷只在前端 ActivityStreamItem（tool_result return null、summarizeToolInput 截斷）、後端資料完整（ipc.ts:557-560）、foldTimeline 只折 thought（pairing 是 net-new）、6 surface 共用、ThoughtItem 是展開範本。全屬實。新發現：T2 與 PE2-C2 順序耦合——codex 編輯(apply_patch)目前無 event，T2 對 codex goal/fix 沒東西可展開，建議 PE2-C2 先行或併同。
- 產出: docs/2026-05-22-app-stream-verbose-spike.md
- 下一步: 佇列下一個 TODO 是 T3（chat-display-polish spike）。

## 2026-05-22 18:43Z — T1 spike settings-chat-model
- 狀態: DONE
- 做了: 盤 chat model/effort 解析。發現 backlog 已部分過時：方案 A（read-only hint）在 Claude 已實作（EndpointSection.tsx:240 endpoint-chat-row），只缺 Codex 端（CodexEndpointSection.tsx 無 chat 列）。Verb::Verify 是方案 B 的現成範本（不 fallback 的 per-verb 子塊）。方案 B 因 codex 加入範圍變大（兩 provider 都要加 chat 子塊），給了完整 file-level 清單。
- 產出: docs/2026-05-22-settings-chat-model-spike.md
- 下一步: 佇列下一個 TODO 是 T2（app-stream-verbose-detail spike）。

## 2026-05-22 18:33Z — PE2 設計 provider prompt 策略
- 狀態: DONE
- 做了: 依 PE1 設計兩條修法。新確認兩個縮小範圍的事實：(1) CLAUDE.md/AGENTS.md 都由 test-enforced 的 NEUTRAL_RULES 產生 → 不用動，C1 只集中在 skill_bundle stub_content；(2) render 只 match 4 個 variant 且靠 name=="Edit" 觸發編輯渲染 → C2 只擴 codex_parser 即可重用渲染，零跨 crate。建議：C1=skill 機制無關化（輕）、C2=擴 codex parser 認 apply_patch/turn.failed（輕-中）。
- 產出: docs/2026-05-22-provider-prompt-design.md
- 下一步: 佇列下一個 TODO 是 T1（settings-chat-model spike）。⚠️ C2 實作卡 ground-truth：spike 從未錄到 codex 編輯/失敗的 --json 樣本，需一次真實 codex 跑（留給 harry）。

## 2026-05-22 HH:MM — PE1 診斷 Codex 輸出成因
- 狀態: DONE
- 做了: 讀 agent/stream/skill_bundle 層，比對 claude vs codex 指示材料 + parser 保真度。發現：(1) skill bundle 與 AGENTS.md 對 codex 是 byte-identical 沿用 Claude 內容，寫死了 `--tools`/PreToolUse hook/`mcp_*` 等 codex 沒有的機制（quiz 自我驗證契約最受影響）；(2) codex parser 只映 3 種 event，檔案編輯(apply_patch)不可見、turn.failed 靜默吞掉、工具全塌成 "Shell"、無增量串流。修正了 backlog 初步猜測：「答案被當 thought」兩 provider 一致，非 codex 獨有。
- 產出: docs/2026-05-22-provider-prompt-diagnosis.md
- 下一步: PE2 設計（per-provider 指示差異化縫 + codex parser event 覆蓋擴充）。等 harry 補具體樣本以判「模型行為差異」類別。

## 2026-05-22 — 加入 PE1/PE2（Codex prompt engineering）
- 狀態: DONE
- 做了: 依 harry 需求把「Codex 整合後輸出不理想」的 prompt engineering 研究排進佇列最前面（PE1 診斷 → PE2 設計），並建 backlog 文件。
- 產出: docs/2026-05-22-provider-prompt-engineering-backlog.md, 更新 PLAN.md + BACKLOG.md
- 下一步: 首輪從 PE1（診斷成因）開始。

## 2026-05-22 — 初始化
- 狀態: DONE
- 做了: 建 loop PLAN + WORKLOG，定下「只讀 + 寫 doc」自主邊界。
- 產出: docs/loop/PLAN.md, docs/loop/WORKLOG.md
- 下一步: 首輪從 T1（settings-chat-model spike）開始。
