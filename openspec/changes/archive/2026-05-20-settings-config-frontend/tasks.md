## 1. i18n 字串資源

- [x] [P] 1.1 在 `codebus-app/src/i18n/`（zh-tw 與 en）新增本 change 所有新欄位的 key：PII on-hit label/選項/Critical-floor 說明、PII extra patterns label/新增/刪除/regex 錯誤、lint.fix.enabled label、quiz/goal content_verify label 與成本提示、log 停用控制 label、chat 唯讀列「沿用 query（{model} / {effort}）」模板。行為：所有新 UI 文案皆可由 `useT()` 解析、zh-tw/en 各一份且 key 對齊。驗證：`npm run typecheck` 乾淨，且 `codebus-app/src/i18n/` 既有 i18n 測試（messages 對齊類）綠。

## 2. SettingsModal 新增 config 欄位（spec: Global Settings Modal Field Set）

- [x] 2.1 在 `SettingsModal.tsx` 加 `pii.on_hit` Select（值 `warn`/`skip`/`mask`，預設 `warn`），含 Critical-floor 說明文案。行為：選值經 `update({ pii: { on_hit } })` 進 dirty payload；UI 顯示「Critical 級永遠 mask、不受此設定影響」文案。驗證：`SettingsModal.test.tsx` 新增測試斷言三個選項存在、選 `skip` 後 save payload 含 `pii.on_hit: "skip"`、Critical 說明文字 render。
- [x] 2.2 在 `SettingsModal.tsx` 加 `lint.fix.enabled` Toggle（預設 true）。行為：切換經 `update({ lint: { fix: { enabled } } })` 進 dirty payload。驗證：`SettingsModal.test.tsx` 斷言預設呈現 enabled、關閉後 save payload 含 `lint.fix.enabled: false`。
- [x] 2.3 在 `SettingsModal.tsx` 加 `quiz.content_verify` 與 `goal.content_verify` 兩個 Toggle（預設皆 false），各自顯示「開啟會多花 verify/repair spawn」成本提示文案。行為：切換分別寫入 `quiz.content_verify` / `goal.content_verify` dirty payload；兩段成本提示文字 render。驗證：`SettingsModal.test.tsx` 斷言兩 toggle 預設 false、開啟後 save payload 各含對應 key true、兩段成本提示文字存在。
- [x] 2.4 在 `SettingsModal.tsx` 既有 Log sink 區塊加「停用 logging」控制，啟用時 save payload 寫 `log.sink: "none"`；ResetButton 邏輯涵蓋此狀態。行為：啟用停用控制 → payload `log.sink` 為 `"none"`；未啟用維持既有 jsonl/dir 行為不回歸。驗證：`SettingsModal.test.tsx` 斷言啟用停用控制後 save payload `log.sink === "none"`，且既有 log dir picker 測試仍綠。

## 3. SettingsModal PII extra patterns 編輯器

- [x] 3.1 在 `SettingsModal.tsx` 加 `pii.patterns_extra` 編輯器：純 regex 字串列表（新增 / 刪除，無 per-entry label），對齊 `codebus-core/src/config/pii.rs` 的 `Vec<String>`。行為：列表內容寫入 `pii.patterns_extra` dirty payload（字串陣列，非物件）。驗證：`SettingsModal.test.tsx` 斷言新增兩條 pattern 後 save payload `pii.patterns_extra` 為對應字串陣列、刪除一條後陣列縮短。
- [x] 3.2 對 `pii.patterns_extra` 每筆輸入做即時 regex 驗證，無效 pattern 顯示 inline 錯誤並使 Save 按鈕 disabled，直到修正或移除。行為：存在無效 regex 時 Save 不可點；全部有效時 Save 恢復可點。驗證：`SettingsModal.test.tsx` 斷言輸入 `[`（非法 regex）→ inline 錯誤 render 且 `settings-save` 按鈕 disabled；改為合法後按鈕恢復 enabled。

## 4. EndpointSection chat 唯讀列

- [x] [P] 4.1 在 `EndpointSection.tsx` 新增不可編輯的 `chat` 列，顯示「沿用 query（{model} / {effort}）」，與 query 列 model/effort 即時聯動；不寫任何 `chat` key 進 payload。行為：query 列改 model/effort 後 chat 列顯示同步更新；save payload 不含 `claude_code.*.chat`。驗證：新增/擴充 EndpointSection 測試斷言 chat 列文字隨 query 值變動、save payload 無 chat key、chat 列無可編輯控件。

## 5. 文件修正

- [x] [P] 5.1 修正 `docs/2026-05-14-pii-settings-ui-backlog.md` 的 schema 段：將誤寫的 `pii.extra_rules`（`{label, pattern}` 物件陣列）更正為實作真實的 `pii.patterns_extra`（純 regex 字串陣列），並註明「無 label」決策來源為 2026-05-19 discuss。行為：該 backlog 文件 schema 描述與 `codebus-core/src/config/pii.rs` 一致。驗證：人工 diff 確認 `pii.patterns_extra` 字串陣列描述、移除 `extra_rules` 物件 schema、保留交叉引用至 `docs/2026-05-19-settings-config-coverage-backlog.md`。

## 6. 收尾驗證

- [x] 6.1 全前端驗證綠，且驗證 spec requirement「Global Settings Modal Field Set」新欄位集合與「Forbidden Behaviors in v1」的「Settings modal has no theme or language controls」場景（modal 僅含定義欄位 + CLI Status + Endpoint Section，無 theme/language 控件）皆成立。行為：本 change 所有新欄位有測試覆蓋且不破壞既有 Settings 行為、無 theme/language 控件回歸。驗證：`npx vitest run`（含新增 `SettingsModal.test.tsx` / EndpointSection 測試與既有 `codebus-app/src/test/forbidden-behaviors.test.tsx`）0 failed、`npm run typecheck` 乾淨；人工確認後端 `codebus-app/src-tauri/src/ipc/config.rs` 與 `codebus-core/src/config/*` 無 diff（本 change backend 不動契約）。
