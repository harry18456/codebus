---
title: Resolver Manifest
type: entity
sources:
  - path: crates/uv-resolver/src/manifest.rs
    sha256: 1e6ddbb0c8c227112789c37397a8b752f14d6418667f51eb81636e9d34aa8456
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
goals:
  - 了解 uv-resolver 的核心 API 跟 entry point
created: '2026-05-05'
updated: '2026-05-05'
related:
  - '[[uv-resolver]]'
  - '[[resolver-options]]'
  - '[[resolver-resolve]]'
stale: false
---

# Manifest

`uv_resolver::Manifest` 是「使用者意圖 → resolver」的最主要輸入容器。一切
從專案、CLI、或 lockfile 解出來的 requirements / constraints / overrides
都包進這個 struct，再交給 `Resolver::new(manifest, ...)` 啟動解析。

## 欄位

```rust
// from crates/uv-resolver/src/manifest.rs
pub struct Manifest {
    pub(crate) requirements: Vec<Requirement>,
    pub(crate) constraints: Constraints,
    pub(crate) overrides: Overrides,
    pub(crate) excludes: Excludes,
    pub(crate) preferences: Preferences,
    pub(crate) project: Option<PackageName>,
    pub(crate) workspace_members: BTreeSet<PackageName>,
    pub(crate) exclusions: Exclusions,
    pub(crate) lookaheads: Vec<RequestedRequirements>,
}
```

| 欄位                | 用途 |
| ------------------- | ---- |
| `requirements`      | 直接需求（pyproject / requirements.txt 第一手列表）。 |
| `constraints`       | 限制條件（pip 的 `-c constraints.txt`）。會疊加在所有 requirement 上但不引入新節點。 |
| `overrides`         | 覆寫條件（uv 特有）。強制取代符合名字的 requirement。 |
| `excludes`          | 直接從 transitive graph 中剔除的套件名集合。 |
| `preferences`       | 偏好版本來源（lockfile pin / 已裝 venv 中的版本）。並非硬限制，但選版會優先採用。詳見 [[uv-resolver]] 列出的 `Preferences`。 |
| `project`           | 當前專案名（若是 workspace 解析）。 |
| `workspace_members` | 同 workspace 內的套件名 set，避免 self-resolve。 |
| `exclusions`        | 「即使裝在環境裡也不要採信」的 `Exclusions`，常用來表達「升級 / 重裝」意圖。 |
| `lookaheads`        | 預先掃過的 transitive 需求（`RequestedRequirements`），用來在解析前就決定 yanked / pre-release / direct-URL 的允許範圍。 |

## 建構 API

兩個建構入口：

```rust
// from crates/uv-resolver/src/manifest.rs
impl Manifest {
    pub fn new(
        requirements: Vec<Requirement>,
        constraints: Constraints,
        overrides: Overrides,
        excludes: Excludes,
        preferences: Preferences,
        project: Option<PackageName>,
        workspace_members: BTreeSet<PackageName>,
        exclusions: Exclusions,
        lookaheads: Vec<RequestedRequirements>,
    ) -> Self { ... }

    pub fn simple(requirements: Vec<Requirement>) -> Self { ... }

    #[must_use] pub fn with_constraints(self, ...) -> Self { ... }
    #[must_use] pub fn with_lookaheads(self, ...) -> Self { ... }
}
```

- `Manifest::simple(requirements)`：所有其他欄位用 `default()`，
  測試與最小範例的首選。
- `Manifest::new(...)`：完整建構式，CLI 端用。
- `with_*` builder pattern 是後續微調，回傳 `Self`。

## 對外的查詢方法

`Manifest` 不只是 dumb data；它提供針對 `ResolverEnvironment` + `DependencyMode`
過濾、套用 overrides 與 markers 的 iterator API：

| 方法                                  | 回傳                                      | 用在哪 |
| ------------------------------------- | ----------------------------------------- | ------ |
| `requirements(env, mode)`             | requirements + overrides 串起來，順序：requirement → override | 主迴圈拉取最終要解的需求集 |
| `requirements_no_overrides(env, mode)`| 同上但不含 overrides                      | 區分用戶輸入與覆寫時 |
| `overrides(env, mode)`                | 只有 overrides                            | yank / pre-release / URL 允許判斷 |
| `user_requirements(env, mode)`        | 「user-facing」需求（含 lookahead.direct）| `lowest-direct` 解析模式 |
| `direct_requirements(env)`            | 只剩直接 requirements + override          | dev-dependency 啟用判斷 |
| `apply(reqs)`                         | 把 overrides 與 constraints 套到任意 reqs | 內部工具 |
| `num_requirements()`                  | `requirements.len()`                      | 統計 |

`DependencyMode::Transitive` vs `Direct` 決定要不要包入 `lookaheads`，
而 `ResolverEnvironment::marker_environment()` 提供 marker 求值用的環境
（universal 模式則回傳 `None`，所有 marker 都當「可能成立」）。

## 與 `Resolver` 的銜接

`Resolver::new_custom_io` 在組 `ResolverState` 時會把 `Manifest` 大部分
欄位逐一 move 進去：

```rust
// from crates/uv-resolver/src/resolver/mod.rs
let state = ResolverState {
    selector: CandidateSelector::for_resolution(&options, &manifest, &env),
    urls: Urls::from_manifest(&manifest, &env, git, options.dependency_mode),
    indexes: Indexes::from_manifest(&manifest, &env, options.dependency_mode),
    project: manifest.project,
    workspace_members: manifest.workspace_members,
    requirements: manifest.requirements,
    constraints: manifest.constraints,
    overrides: manifest.overrides,
    excludes: manifest.excludes,
    preferences: manifest.preferences,
    exclusions: manifest.exclusions,
    ...
};
```

亦即 `Manifest` 自身在 `Resolver::new` 之後不再被持有；它只是把欄位拆開
餵給內部 state，接著由各種策略物件（`CandidateSelector`、`Urls`、
`Indexes`、`AllowedYanks`）各取所需。
