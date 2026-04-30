## Context

D-020 把「Module 6 介入控制器」的介面契約延後到前端動工時再決定，原因是 backend 三支 API（`/scan` / `/explore` / `/generate`）已涵蓋介入需要的能力，前端組合 + 既有 progress.json / route.json 寫入路徑就能把 MVP 三介入點長出來。Phase 6 step 29 是兌現節點：R-01 station board / Explorer console / Q&A drawer 都已上線，三介入點需要落在哪幾條 UX surface、新加哪些檔案、後端只擴 `target_stations` arg 而非新 endpoint，都已經能在現有架構上拍板。

本 design 鎖三個關鍵技術決策的細節 + 兩個跨層 invariant，避免 apply 期反覆撞牆：

1. `target_stations` partial regen 的覆寫範圍與 MOC / route.json invariant
2. `progress.json` 的 `skipped_station_ids` schema migration 與解鎖規則
3. Switch workspace 與 grant flow 的互動

其他細節（confirm modal copy、SSE event payload shape）走 spec scenarios 寫，不在此重複。

## Goals / Non-Goals

**Goals:**

- 三介入點皆走 confirm-modal-then-action pattern，UX 一致
- progress.json schema 變動 additive、不破舊檔（backward-compatible read）
- Per-station regen 不破壞 unrelated stations 的 markdown / frontmatter / order；MOC 與 route.json 的 station 順序與 station_id 集合維持不變
- 三介入點共用一個 composable（`useIntervention`），避免 modal state 各自為政
- defensive 測覆蓋三條 flow 的關鍵 invariant（progress 寫入正確性 / partial regen 不誤刪 / switch 不丟進度）

**Non-Goals:**

- 不解 Phase 2 KB ops UI（`docs/qa-agent.md §十` defer）
- 不引入 Pinia 或其他 state container（沿用 composable + module-level singleton）
- 不重畫 design/v1/ mockup 補三介入點（mockup 收到 14 為止，本 change UX 直接跟 docs 對齊）
- 不改 Module 5 generator 的 LLM prompt / Validator 邏輯（partial regen 只是控制覆寫範圍）
- 不做 station-level 重生失敗 rollback（`/generate` 既有錯誤碼 `GENERATE_FAILED` 已足，前端顯示錯誤訊息即可）

## Decisions

### Decision 1: `target_stations` partial regen 的覆寫範圍

**選擇**：`target_stations` 只覆寫命中站的 station markdown 檔（`stations/s{NN}-slug.md`）+ frontmatter；**不重生** MOC（`tutorial.md`）、**不重生** `route.json`、**不改變** unrelated stations 的任何檔案；命中站之間的 station_id 順序與 stable_id 必須與原 route.json 一致（partial regen 不允許重排）。

**為何不選**：
- (X) 全 tutorial 重生（默認行為）：UX 不一致，「重生此站」變「重生整份」，使用者預期落差大
- (X) target_stations 觸發 MOC 重生：MOC 含 station 描述、completion progress 反查；重生會打掉使用者已寫進去的 checkpoint / quiz 答題狀態（雖然 progress.json 不會丟，但 MOC 內嵌的 mdc 元件 ref 會錯）— 風險高
- (X) target_stations 觸發 route.json 重生：station_id 是 progress.json `completed_station_ids` 的 key；重排會讓進度錯位，違反 progress invariant

**Rationale**：partial regen 的核心需求是「某站內容不滿意 → 換 LLM 重寫此站」，不應該動到全 tutorial 結構。Generator runner 內部本來就分 `tutorial.md` MOC pass + per-station pass + `route.json` pass 三段（per `module-5-generator.md §十四`），partial 模式只跑 per-station pass + 對應命中站的檔案寫入即可。

**Invariant**：partial regen 後 `route.json` 的 `stations[*].station_id` 序列與 `tutorial.md` 的 MOC 連結與 partial regen 前**完全一致**；若 LLM 對命中站重寫產生新的 station_id，runner 必須拒絕並回 `GENERATE_FAILED`（station_id stable invariant 由 generator 既有 `stable_id` 模組保證，partial 模式不放鬆此 invariant）。

### Decision 2: `progress.json` `skipped_station_ids` schema migration 與解鎖規則

**選擇**：progress.json 加 `skipped_station_ids: string[]`（separate list）；解鎖規則改為 `unlocked_set = completed_station_ids ∪ skipped_station_ids`；舊 progress.json（無此欄）讀取時自動 fallback `skipped_station_ids: []`，不需顯式 migration step。

**為何不選**：
- (X) 把 skip 塞進 `completed_station_ids` 並加 `mode` 欄：要把現有 `string[]` 改成 `Array<{id, mode}>`，破舊 progress 檔案讀取（per `interactive-tutorial/spec.md` Requirement「progress.json schema and single-writer path」），需要顯式 schema migration、得改 `useTutorialProgress` 寫入路徑
- (X) 用 checkpoint 內的 hidden flag：語意混淆，checkpoint 是 mdc 元件互動結果，與站級 skip 概念不同層

**Rationale**：additive list 是最低風險 migration。命名 `skipped_station_ids` 與 `completed_station_ids` 平行，schema 自我文件化。解鎖判定改一個 set union 即可，不影響既有 unlock invariant（per `interactive-tutorial/spec.md` Requirement「Already-completed station revisitability」— 跳過的站同樣 reachable via direct URL paste）。

**Invariant**：
- 一站不能同時在 `completed_station_ids` 與 `skipped_station_ids`（mutual-exclusion；寫入端守）
- 跳過後可重新點開站學；學完按 checkpoint / quiz 完成 → 從 `skipped_station_ids` 移除、加進 `completed_station_ids`（前向轉換 only）
- 不提供「取消跳過」UI（跳過後若想學，直接點開學完即可，不需要顯式 unskip）

### Decision 3: Switch workspace 與 grant flow 的互動

**選擇**：Switch workspace 不嘗試保留 grant 狀態 — 一律 `router.push('/')` 回到 entry page，新 workspace 路徑要重新跑 grant flow（per `openspec/specs/authorization-audit/spec.md`）。同 workspace 路徑回來時，現有 grant 已落 `~/.codebus/authorization_audit.jsonl`，frontend 自動跳過 grant modal、直接進 station board。

**為何不選**：
- (X) Switch 時保留當前 sidecar grant token：grant 是 per-workspace（per `docs/authorization.md §五` workspace_root binding），跨 workspace reuse 違反 D-002 + 雙模 discriminator invariant
- (X) Switch 強制清掉所有歷史 grant：使用者預期「我之前授權過某資料夾，回去學應該不用再 grant」會被打破；不必要的摩擦

**Rationale**：grant 是基於 workspace 路徑的 binding，沒有「全域 session」概念。Router 回 entry page 後，entry page 的既有「workspace pick → 已 grant 偵測 → 跳 station board / 跳 grant modal」決策樹自動處理新舊 workspace。

**Invariant**：
- Switch 不刪 progress.json、不刪 KB（Qdrant collection）— 同 workspace 回來會直接接續
- Switch 不撤銷 grant（不寫 `grant_revoked` event）— 切走不等於不再授權，後續若要正式撤回走另外路徑（不在本 change 範疇）

### Decision 4: `useIntervention` composable 的 state ownership

**選擇**：`useIntervention` 是 module-level singleton（同 `useQaSession`）；維護一個 `pendingAction: { kind: 'skip' | 'regen' | 'switch', payload, onConfirm } | null` ref。`InterventionConfirmModal` 訂閱此 ref，render 對應 modal 內容；`SkipStationButton` / `RegenStationButton` / `SwitchWorkspaceMenu` 三個 leaf 元件 imperative 呼 `useIntervention.requestSkip(stationId)` / `requestRegen(stationId)` / `requestSwitch()`。

**為何不選**：
- (X) 每個 leaf 元件自己管 modal state：3 個 modal 各自 mount，可能同時開；UX 混亂
- (X) 用 v-model 把 modal state 提到 page level：要 prop drill 到深層 station-page 組件樹，違反 dumb + emit pattern
- (X) Pinia store：超出 D-026 既有 stack，引入 14KB 新依賴 + 配置成本，沒必要

**Rationale**：與 Phase 6 既有 `useQaSession` / `useExplorerStream` 慣例一致。Confirm modal 同一時刻只渲染一個（`pendingAction` 是 single ref，不是 list），UX 一致。

## Risks / Trade-offs

- **Risk: Per-station regen 的 LLM 對命中站給出的 station_id 與原 route.json 不符**（例如改寫過程裡換了 slug）→ Mitigation: generator runner 在 partial 模式下守「命中站的 generated station_id == request.target_stations[i]」invariant，不符直接 fail（`GENERATE_FAILED` + 錯誤訊息指明 station_id mismatch）；前端 confirm modal 文案明示「重生失敗時此站保留原樣」。
- **Risk: 使用者快速連按「↷ 跳過」造成多次 progress.json 寫入競態** → Mitigation: `useTutorialProgress` 既有 500ms debounce + beforeunload flush 機制覆蓋；`useIntervention.requestSkip` 進 confirm modal 後才寫，加上 modal 的 confirm-once 守則，competing writes 不會發生。
- **Risk: Switch workspace 後使用者立刻 Cmd+K 召喚 Q&A drawer，但 drawer state 還指向舊 workspace** → Mitigation: `useQaSession` 已監聽 workspace 變更（後續會 reset session）；本 change 不負責此處清理，但 design 留下 follow-up 給 P1（若 e2e 發現 leak 才補）。
- **Trade-off: 不做 reorder / 加自訂站、不做 unskip UI**：scope 收緊到「最小可 demo 可用」，UX 可能在 demo 期被問到（「能不能改順序？」）；接受此 trade-off，per AskUserQuestion 對齊 Non-Goals 已記。
- **Trade-off: `target_stations` 是 list 而非 single station**：保留未來擴展「重生多站」的能力，但 P0 前端只送 single station。spec scenarios 涵蓋 single + multi 兩條，避免 future change 改 schema。

## Open Questions

- (Phase 2) Reorder 怎麼設計？需要動 route.json + progress.json 兩份，且要與 Generator 的 station_id stable invariant 共存 — 留 ADR 評估。
- (P1+) Unskip UI 要不要做？若使用者反映「跳過後後悔」是高頻問題就做；目前接受「跳過後可重新點開學完」的前向轉換為唯一路徑。
- (Phase 2) Switch workspace 的 audit log 要不要寫？目前不寫（不算 grant 變動），但若需要 telemetry 追蹤使用模式可以加 `interaction_audit.jsonl` 第八層 — 留 P1 評估。
