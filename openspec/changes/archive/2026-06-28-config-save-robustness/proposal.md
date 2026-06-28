## Why

`~/.codebus/config.yaml` 的 pii.patterns_extra 若含一個空字串 pattern，PII 掃描會在每個字元位置都命中一個 zero-width（空）match——大型檔案（實測 CodeCop 的 241KB markdown）因此產生數十萬筆 match，`codebus init` / raw mirror sync 卡住 2 分鐘跑不完。空 pattern 的來源是 app Settings 的「新增 PII 規則」允許留空白行、儲存時也沒過濾。同一條「app 儲存設定」路徑上還有第二個缺陷：儲存時以 serde_yaml 重新序列化整份 config，會洗掉 starter config 手寫的教學註解。兩者同源，一併修掉。

## What Changes

- **PII 空 / zero-width pattern 防爆（核心 bug）**：scanner 掃描時略過 zero-width（start == end）match，從根本擋掉任何 zero-width pattern（空字串、a*、\b…）造成的逐字元爆量；scanner 建構時略過空 / 純空白的 patterns_extra 條目（不編譯、不成為規則）。
- **空 pattern 來源過濾（縱深防禦）**：app 後端 save_global_config 與前端 settings store 的 save() 都在寫入前濾掉空 / 純空白的 patterns_extra，使空 pattern 永不落地。
- **config 不再帶 inline 教學註解**：starter config 改為純值 + 一段極簡共用 header（指向文件）；app 儲存時重貼同一段 header（單一來源常數，starter 與 save 共用）。欄位教學移到新文件 docs/config-reference.md，不再依賴會被序列化洗掉的 inline 註解。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `pii-filter`: 新增「空 / zero-width 額外 pattern 安全」requirement——scanner 建構略過空 pattern、掃描略過 zero-width match，兩者皆有測試。
- `cli`: 新增「Global config starter 內容形態」requirement——starter 為純值 + 共用 header、無 inline 教學註解，欄位教學改放 docs/config-reference.md；round-trip-to-defaults 不變。
- `app-shell`: 新增「Config 儲存衛生」requirement——save_global_config 濾空 patterns_extra 並 prepend 共用 header；Settings UI 送出前也濾空。

## Impact

- Affected specs: pii-filter, cli, app-shell（皆新增 requirement，無移除）
- Affected code:
  - Modified:
    - codebus-core/src/pii/scanners/regex_basic.rs — scan 略過 zero-width match、new 略過空 / 純空白 patterns_extra，加對應測試
    - codebus-core/src/config/global_starter.rs — 抽出單一來源 CONFIG_HEADER 常數、STARTER_CONFIG 改純值 + header，更新 doc 註解與測試
    - codebus-core/src/config/mod.rs — re-export CONFIG_HEADER
    - codebus-app/src-tauri/src/ipc/config.rs — save 前濾空 patterns_extra、寫檔前 prepend CONFIG_HEADER，加測試
    - codebus-app/src/store/settings.ts — save() 送出前濾空 patterns_extra，加測試
  - New:
    - docs/config-reference.md — config 欄位教學的穩定載體（取代被洗掉的 inline 註解）
  - Removed: (none)
