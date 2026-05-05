---
title: "了解 uv-resolver 的核心 API 跟 entry point"
goal: "了解 uv-resolver 的核心 API 跟 entry point"
created: '2026-05-05'
updated: '2026-05-05'
---

# 閱讀指引：uv-resolver 的核心 API 跟 entry point

## 想搞懂的問題

- `uv-resolver` 在 uv 軟體棧的位置是什麼？對外公開哪些型別？
- 想呼叫它解析一組 requirements，最少要準備什麼？
- 主流程怎麼運作？為什麼程式裡同時看到 tokio 與 `std::thread`？
- 解出來的「結果」長什麼樣？怎麼接下游？

## 建議閱讀順序

1. **[[uv-resolver]]** ── 從 crate 級總覽抓住模組地圖與公開 API 表面
   （`pub use` 列表）。先掃過 `src/lib.rs` 的匯出再決定要深入哪個。

2. **[[resolver-manifest]]** ── 知道 resolver 的「輸入結構」：
   requirements / constraints / overrides / preferences / lookaheads
   分別代表什麼，以及 `Manifest::simple` 與 `Manifest::new` 的差異。

3. **[[resolver-options]]** ── 補齊「策略 knob」：`ResolutionMode`、
   `PrereleaseMode`、`DependencyMode`、`ForkStrategy`、`ExcludeNewer`，
   以及 `Flexibility` 在 lockfile 重放時的角色。

4. **[[resolver-resolve]]** ── 跟著 `Resolver::new` → `with_reporter` →
   `resolve()` 一路看完整個雙線程設計：tokio 上的 fetcher + 獨立 OS
   thread 上的 PubGrub solver，靠 mpsc channel 與 `InMemoryIndex` 通訊。

5. **[[resolver-provider]]** ── 想客製 IO（mock / 自訂 registry / 測試
   fixture）時看這頁。trait 三個方法 + `DefaultResolverProvider` 的
   錯誤分類、Reporter facade 的轉接邏輯。

6. **[[resolver-output]]** ── 收尾：`ResolverOutput` 內藏的
   `petgraph::Graph<ResolutionGraphNode, UniversalMarker>`、
   `AnnotatedDist` 的欄位，以及「marker 表達式怎麼跟著節點走」。

## 跳到原始碼的小抄

| 想看的東西 | 路徑 |
| ---------- | ---- |
| 公開 API 全貌 | `crates/uv-resolver/src/lib.rs` |
| Entry point | `crates/uv-resolver/src/resolver/mod.rs::Resolver::{new, new_custom_io, with_reporter, resolve}` |
| PubGrub 主迴圈 | `crates/uv-resolver/src/resolver/mod.rs::ResolverState::solve` |
| 輸入容器 | `crates/uv-resolver/src/manifest.rs` |
| 策略開關 | `crates/uv-resolver/src/options.rs` |
| IO trait | `crates/uv-resolver/src/resolver/provider.rs` |
| 進度回呼 | `crates/uv-resolver/src/resolver/reporter.rs` |
| 輸出圖 | `crates/uv-resolver/src/resolution/{mod,output}.rs` |

## 沒涵蓋到、想再深入時的方向

- `pubgrub/` 子模組與自訂的 `PubGrubPackage` / `PubGrubPriorities` 設計。
- Universal mode 的 fork 演算（`resolver/environment.rs`、`fork_*.rs`、
  `universal_marker.rs`）。
- `lock/` 子模組怎麼把 `ResolverOutput` 序列化成 `uv.lock` /
  `pylock.toml` / `requirements.txt`。
- 錯誤回報路徑：`error.rs` 的 `NoSolutionError` + `report.rs` 的 PubGrub
  decision tree 顯示。
- `BatchPrefetcher` 與 `InMemoryIndex` 的快取與並發模型細節。
