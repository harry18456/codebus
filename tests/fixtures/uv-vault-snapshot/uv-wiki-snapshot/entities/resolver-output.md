---
title: Resolver Output
type: entity
sources:
  - path: crates/uv-resolver/src/resolution/mod.rs
    sha256: 7c4f624dc23a1177b84288dd7946c89f6f7d47901b8d5e825d6ea3bc96b2b37d
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-resolver/src/resolution/output.rs
    sha256: 28b6520922c047d5d823ab6fdf90c93bfebc98f48b40c649bd7ef0b832fd811f
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
goals:
  - 了解 uv-resolver 的核心 API 跟 entry point
created: '2026-05-05'
updated: '2026-05-05'
related:
  - '[[uv-resolver]]'
  - '[[resolver-resolve]]'
stale: false
---

# ResolverOutput / AnnotatedDist

`Resolver::resolve()` 成功時回傳 `Result<ResolverOutput, ResolveError>`，
這是整個解析流程對外的「成功型別」。下游（lockfile 序列化、安裝器、
display）都接這個型別繼續處理。

## ResolverOutput

```rust
// from crates/uv-resolver/src/resolution/output.rs
pub struct ResolverOutput {
    pub(crate) graph: Graph<ResolutionGraphNode, UniversalMarker, Directed>,
    pub(crate) requires_python: RequiresPython,
    pub(crate) fork_markers: Vec<UniversalMarker>,
    pub(crate) diagnostics: Vec<ResolutionDiagnostic>,
    pub(crate) requirements: Vec<Requirement>,
    pub(crate) constraints: Constraints,
    pub(crate) overrides: Overrides,
    pub(crate) options: Options,
}
```

| 欄位             | 內容 |
| ---------------- | ---- |
| `graph`          | 一張 [`petgraph`](https://docs.rs/petgraph/) 的有向圖：節點是 `ResolutionGraphNode`（root 或一個 `AnnotatedDist`），邊是 `UniversalMarker`（這條相依在哪個 marker 集合下成立）。 |
| `requires_python`| 整個解出的 graph 對應的 Python 版本範圍。 |
| `fork_markers`   | 若解析過程有 non-identical fork（universal mode），這裡保留各個 fork 的 marker，下次重放時可重建相同分支。 |
| `diagnostics`    | 過程中產生的非致命警告（yanked、incompatible markers 等）。 |
| `requirements` / `constraints` / `overrides` / `options` | 把建構時用到的輸入一起留存，用於後續 lockfile 寫入或 derivation chain 報錯。 |

注意所有欄位都是 `pub(crate)`，外部要走 crate 內提供的轉換器（例如
`Lock::try_from(ResolverOutput)`、`DisplayResolutionGraph::new`）才能取出
資訊。`ResolverOutput` 比較像「不可變的快照」，不直接對外暴露結構細節。

## ResolutionGraphNode

```rust
// from crates/uv-resolver/src/resolution/output.rs
pub(crate) enum ResolutionGraphNode {
    Root,
    Dist(AnnotatedDist),
}
```

`Root` 是虛擬根，代表「使用者意圖」這個邏輯起點；其他都是 `Dist`，
裡面是包好的 `AnnotatedDist`。

## AnnotatedDist

```rust
// from crates/uv-resolver/src/resolution/mod.rs
pub(crate) struct AnnotatedDist {
    pub(crate) dist: ResolvedDist,
    pub(crate) name: PackageName,
    pub(crate) version: Version,
    pub(crate) extra: Option<ExtraName>,
    pub(crate) group: Option<GroupName>,
    pub(crate) hashes: HashDigests,
    pub(crate) metadata: Option<Metadata>,
    pub(crate) marker: UniversalMarker,
}
```

| 欄位       | 意義 |
| ---------- | ---- |
| `dist`     | 真正可下載 / 可使用的 distribution（已選好的 wheel/sdist 或安裝中的版本）。 |
| `name`     | 套件名（lower-case 正規化）。 |
| `version`  | 解出的 `Version`。 |
| `extra` / `group` | 若節點代表一個 extra 啟用 (`pkg[foo]`) 或 dependency-group，則指明是哪一個。 |
| `hashes`   | 已知的 wheel/sdist hash 摘要，可寫入 lockfile。 |
| `metadata` | 解析過程中拿到的 `Metadata`（PKG-INFO / METADATA），有則順便保存。 |
| `marker`   | 「這個 distribution 在哪些 marker 環境下可被安裝」的 universal marker——這是把整張 graph 從 root 走到此節點的所有路徑做 disjunction 的結果。 |

`AnnotatedDist::is_base()`：節點是否為「基礎節點」（沒有 extra / group）。
`AnnotatedDist::index()`：若 dist 來自某個 registry，回傳 `IndexUrl`。

## 讀者導引

- 想 **顯示** 給使用者：用 `DisplayResolutionGraph` + `AnnotationStyle`
  （`uv pip compile` 走這條）。
- 想寫成 lockfile：用 `Lock::from_resolution(...)`（在 `lock/mod.rs` 中）。
- 想偵錯為什麼選了某版本：用 `DerivationChainBuilder`，詳見
  [[uv-resolver]] 模組總覽。
- 想把它丟給安裝器：再經 `Resolution`（distribution-types crate 中的型別）
  下發。

`ResolverOutput` 本身偏「不可變、容易序列化」設計，所有後續處理都從
這張 `petgraph::Graph` 衍生而來。
