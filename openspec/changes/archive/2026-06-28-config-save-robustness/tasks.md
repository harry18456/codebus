## 1. PII scanner 空 / zero-width 防爆（codebus-core）

實作 spec「Empty and Zero-Width Extra Pattern Safety」，落實 design 決策「層 1：scanner 掃描略過 zero-width match」與「層 2：三處過濾空 / 純空白 patterns_extra」的 scanner 端兩處。

- [x] 1.1 [P] RegexBasicScanner::scan 略過 zero-width match（start == end 不 emit），使任何 zero-width pattern 不再逐字元爆量。驗證：新增 unit test——`[""]` + 25 萬字內容回 0 match、`a*` 對非空內容回 0 match；`cargo test -p codebus-core regex_basic` 綠。滿足 spec「Empty and Zero-Width Extra Pattern Safety」。
- [x] 1.2 RegexBasicScanner::new 略過 trim().is_empty() 的 patterns_extra 條目（不編譯、不成規則），custom-N 只對非空連續編號。驗證：unit test new(["".into()]) 後規則數 == builtin_pattern_count()、new(["", "\\bINTERNAL-\\d{6}\\b"]) 命中 pattern_name 為 custom-0；既有 custom_pattern_triggers_via_patterns_extra 仍綠。

## 2. CONFIG_HEADER 單一來源 + starter 純值（codebus-core）

實作 spec「Global Config Starter Content Shape」，落實 design 決策「CONFIG_HEADER 編譯期單一來源（macro_rules!）」與「starter 改純值、教學移至 docs/config-reference.md」的 codebus-core 端。

- [x] 2.1 [P] 在 global_starter 以 macro_rules! 定義單一來源 header、新增 `pub const CONFIG_HEADER`，STARTER_CONFIG = concat!(header, 純值 body)，移除所有 inline 欄位教學註解但保留實際 key: value。驗證：unit test STARTER_CONFIG.starts_with(CONFIG_HEADER)、既有 starter_round_trips_to_defaults 與 schema 子字串測試仍綠、新斷言 body 不含某段舊 inline 教學字串。滿足 spec「Global Config Starter Content Shape」。
- [x] 2.2 從 codebus_core::config re-export CONFIG_HEADER（config/mod.rs）供 app 端引用。驗證：`cargo build -p codebus-core` 綠；由 task 4.2 在 config.rs 成功 import `codebus_core::config::CONFIG_HEADER` 實證可解析。

## 3. config 欄位教學文件

延續 spec「Global Config Starter Content Shape」與 design 決策「starter 改純值、教學移至 docs/config-reference.md」，把被移除的 inline 教學落到穩定載體。

- [x] 3.1 [P] 建立 docs/config-reference.md，把原 starter inline 教學（pii、agent system + azure 範例、hooks、lint、log 各 knob 說明）整理成欄位 reference，CONFIG_HEADER 的 doc-pointer 指向此路徑。驗證：內容 review——涵蓋原 starter 所有欄位教學無遺漏、header 指向的路徑與實際檔名一致。

## 4. app 儲存衛生（codebus-app）

實作 spec「Config Save Hygiene」，落實 design 決策「層 2：三處過濾空 / 純空白 patterns_extra」的後端過濾與「save 重貼 header（字串拼接，非 YAML round-trip）」。

- [x] 4.1 [P] save_global_config 寫檔前濾掉 pii.patterns_extra 的空 / 純空白條目，空 pattern 永不落地。驗證：新 unit test——save(["", "real-pattern"]) → reload 只剩 ["real-pattern"]；既有 save round-trip 測試仍綠。滿足 spec「Config Save Hygiene」。
- [x] 4.2 save_global_config 序列化後 prepend re-export 的 CONFIG_HEADER 再原子寫檔，使 app-saved config 與 starter 同形態。驗證：unit test save 後 on-disk YAML 以 CONFIG_HEADER 起頭且 load_global_config 回讀不報錯；依賴 task 2.2。

## 5. 前端儲存過濾（codebus-app frontend）

延續 spec「Config Save Hygiene」與 design 決策「層 2：三處過濾空 / 純空白 patterns_extra」的前端來源端過濾。

- [x] 5.1 [P] settings store save() 送出 IPC payload 前過濾 config.pii.patterns_extra 的空 / 純空白條目；editor 仍允許新增空行。驗證：新增 settings.test.ts case——含空 patterns 的 config 經 save() 後傳給 saveGlobalConfig 的 payload 已濾空；`npm run test` 綠。

## 6. 驗證與品質

- [x] 6.1 跑 CI 等價全套確認無回歸。驗證：`cargo test -p codebus-core` 與 `cargo test -p codebus-cli` 綠、`cargo clippy --workspace` 無新警告、codebus-app `npm run test` 與 `npm run typecheck` 綠。
- [x] 6.2 以非花費路徑佐證 PII 不再卡：用 task 1.1 的 25 萬字 + 空 pattern unit test（必要時加 raw_sync 等價 integration test）證明掃描完成且 0 爆量 match，不跑真 agent。驗證：對應測試綠並在 apply 報告註明採何種非花費路徑佐證。
