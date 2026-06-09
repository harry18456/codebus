# T12 Spike：RAG index + search（LanceDB）

**Date:** 2026-05-22
**Task:** loop T12（只讀探勘）
**背景:** [rag-index-search backlog](2026-05-14-rag-index-search-backlog.md)（2026-05-14, parked）

---

## TL;DR

backlog 設計完整、無設計缺口。**現況核對:無相關 deps、無 index 模組、wiki 已有頁面 loader 可重用、F 仍未到位**。維持 parked 為主；補三點對 backlog 後續落地有用的觀察。

---

## 現況核對

1. **零 RAG deps**:`grep "lancedb\|onnx\|candle\|tract\|embedding"` 全工作區無命中——`lancedb` + ONNX runtime 都是 net-new heavy dep,影響 build 時間與 binary 大小,動工前要評估接受度。
2. **無 `codebus-core/src/index/` 模組**:backlog `:73` task 3 是 net-new。
3. **wiki 頁面 loader 可重用**:`wiki/lint/rule.rs:54-61 LoadedPage / Vec<LoadedPage>` 已是 lint 用的「全載入 wiki」結構,RAG 的 indexer 可直接餵這條（避免另寫 wiki walker）。算 backlog task 3 的隱形便利。
4. **F 未到位**:同 T11,wiki 內容尚不算「ship 後穩定」,backlog `:92` 「after F」依賴仍卡。

## 對 backlog 的補充觀察

### O1：與 T13(openai-privacy-filter)的 ONNX 共用基礎設施值得提前定案
backlog `:94` 已點到「兩者都用 ONNX runtime → 可共用 build infrastructure」。建議:**先做的那一個來決定 ONNX runtime 怎麼接**(crate 選擇:`ort` vs `tract`、binary 打包策略、跨平台 build),另一個 follow。否則兩條獨立決定 ONNX 路線 → 後者要 rework。誰先做都行,先做的人定基礎設施。

### O2：與 PE2 / per-provider 動的耦合
若採 backlog 提的「query pre-warm」「chat init」做法（spawn 前把 top-K 頁面注入 system prompt）——這條注入路徑同時影響 claude 和 codex。**PE2 設計**目前 C1 建議「skill 機制無關化」,但 RAG 注入的 system prompt 內容也是「指示材料」的一部分；落地時要設計成 **provider-neutral 文字注入**（純 `[[page]]` 引用清單,不點名工具機制),才不會把 PE1 治好的 codex 失準在 RAG 注入處重新製造。建議寫進 backlog tasks。

### O3：incremental 落地路線
照 backlog `:60-66` 的 diff-driven incremental,**先 ship 不含 pre-warm/chat-init 的 standalone `codebus search`**(use case 1) 即可獨立驗 stack(embedding model 整合 + LanceDB upsert),不需動 verb 層。再分階段補 pre-warm(use case 2) 和 chat init(use case 3)。降低「重」工程量被一次到位拖累的風險。

## 依賴與排序

照 backlog `:96-99`(`< 30 頁 parked / > 50 頁起 propose`)維持。建議起 propose 時:
1. 先 standalone `codebus search`(O3)。
2. ONNX runtime 選擇與 T13 對齊(O1)。
3. 注入路徑 provider-neutral(O2)。

## 待 harry
無新阻塞。等 vault 頁面數成長到痛點或 ship app 後使用量起來再起 propose;那時想到 O1-O3。
