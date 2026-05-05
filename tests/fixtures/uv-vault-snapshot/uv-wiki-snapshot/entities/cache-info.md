---
title: CacheInfo struct 與 cache-keys 設定 enum
type: entity
sources:
  - path: crates/uv-cache-info/src/cache_info.rs
    sha256: b877f39ea29ce5bf442af8c95be641336a8dce3ee29dcc82faadac87c3d18880
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-cache-info/src/timestamp.rs
    sha256: e73592bd2b8b13fcd3eade1a9f591c6646e07d3febe3aa39dd4d996ab429ae22
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-cache-info/src/git_info.rs
    sha256: 311af4a0db54b2cd2524354322a2f23be40640095a893f96a2e4d0a0df3a99e2
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
goals:
  - 了解 uv-cache-info 怎麼跟 uv-cache-key 互動
created: '2026-05-05'
updated: '2026-05-05'
related:
  - '[[uv-cache-info]]'
  - '[[cache-info-vs-cache-key]]'
stale: false
---

# CacheInfo struct 與 cache-keys 設定 enum

[[uv-cache-info]] crate 對外暴露兩個容易被混為一談的型別：

- **`CacheInfo`** — runtime 蒐集到的「環境快照」，會被序列化進 cache pointer 檔案。
- **`CacheKey`** — 反序列化 `pyproject.toml` 裡 `[tool.uv].cache-keys` 的單一條目；命名跟 [[uv-cache-key]] 的 `CacheKey` trait 撞車，但意義不同。詳見 [[cache-info-vs-cache-key]]。

## `CacheInfo` 結構

```rust
// from crates/uv-cache-info/src/cache_info.rs
#[derive(Default, Debug, Clone, Hash, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct CacheInfo {
    timestamp: Option<Timestamp>,
    commit: Option<Commit>,
    tags: Option<Tags>,
    #[serde(default)]
    env: BTreeMap<String, Option<String>>,
    #[serde(default)]
    directories: BTreeMap<Cow<'static, str>, Option<DirectoryTimestamp>>,
}
```

五個欄位對應 `cache-keys` 五種設定方式各自的偵測結果：

| 欄位 | 對應 `cache-keys` 設定 | 用來偵測什麼變更 |
|------|------------------------|------------------|
| `timestamp` | `Path(_)` / `File { file }` 跟 glob | 全部相關檔案 mtime/ctime 取最大值；只要有一個檔案改了就會變 |
| `commit` | `{ git = true }` 或 `{ git = { commit = true } }` | 當前 HEAD commit hash（40 hex chars） |
| `tags` | `{ git = { tags = true } }` | `.git/refs/tags` 下所有 tag→commit 對應；新增/刪除/移動 tag 都會被偵測 |
| `env` | `{ env = "VAR_NAME" }` | 環境變數的值（不存在記為 `None`） |
| `directories` | `{ dir = "..." }` | 目錄的 creation time，沒有的話 Unix 退到 inode |

注意：

- 全部欄位都是 `Option`，沒設定的部分留 `None`；序列化時 `#[serde(default)]` 讓舊 cache 檔案在新欄位不存在時依舊能還原。
- `Hash` 雖然有 derive，但實際用法都是 `==` / `!=` 結構比對（見 [[uv-cache-info]] 末段）。
- `PartialEq` 加 `Eq` 才是 cache 失效判斷的核心：`built_wheel_index.rs` 直接拿當前 `CacheInfo` 跟 cache pointer 裡存的 `CacheInfo` 比一次。

### `Timestamp`

```rust
// from crates/uv-cache-info/src/timestamp.rs
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub struct Timestamp(std::time::SystemTime);
```

平台分流：

- Unix：`ctime` + `ctime_nsec`，因為 `ctime` 包括 inode 級的修改（hardlink、權限），比 `mtime` 更保守。
- 其他平台：`metadata.modified()` (`mtime`)。

設計權衡：偵錯漏報好過誤報，所以 Unix 故意選了「會把更多變更視為差異」的 `ctime`。

### `DirectoryTimestamp`

```rust
// from crates/uv-cache-info/src/cache_info.rs
#[derive(Debug, Clone, Hash, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(untagged, rename_all = "kebab-case", deny_unknown_fields)]
enum DirectoryTimestamp {
    Timestamp(Timestamp),
    Inode(u64),
}
```

`untagged` serde 表示序列化進 JSON 時看實際型別決定，反序列化時嘗試兩個 variant。優先用 `Timestamp(creation_time)`，跨平台不可得就退到 `Inode(metadata.ino())`，後者只在 Unix 編譯。

### `Commit` / `Tags`

```rust
// from crates/uv-cache-info/src/git_info.rs
pub(crate) struct Commit(String);
pub(crate) struct Tags(BTreeMap<String, String>);
```

兩者都是 `pub(crate)`，外界透過 `CacheInfo.commit` / `CacheInfo.tags` 間接看到。`Commit` 內部就是 40 個 hex char 的字串（讀完會驗長度跟字元）。`Tags` 用 `BTreeMap<tag_name, commit_hash>`，**選 `BTreeMap` 而非 `HashMap` 是因為前者迭代順序穩定**，這樣序列化跟結構等價判斷才有意義。

## `CacheKey` 設定 enum

```rust
// from crates/uv-cache-info/src/cache_info.rs
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged, rename_all = "kebab-case", deny_unknown_fields)]
pub enum CacheKey {
    Path(Cow<'static, str>),                          // "Cargo.lock" 或 "**/*.toml"
    File { file: Cow<'static, str> },                 // { file = "Cargo.lock" }
    Directory { dir: Cow<'static, str> },             // { dir = "src" }
    Git { git: GitPattern },                          // { git = true } 或 { git = { commit = true, tags = false } }
    Environment { env: String },                      // { env = "UV_CACHE_INFO" }
}
```

這個型別**只是 deserialization shim**，把 `pyproject.toml` 的 `[tool.uv].cache-keys` 陣列每一條對應到一個 enum value。`CacheInfo::from_directory` 會 match 這個 enum 把對應資訊填進 `CacheInfo` 結構。

預設值（`pyproject.toml` 沒寫 `cache-keys` 時用）：

```rust
// from crates/uv-cache-info/src/cache_info.rs
vec![
    CacheKey::Path(Cow::Borrowed("pyproject.toml")),
    CacheKey::Path(Cow::Borrowed("setup.py")),
    CacheKey::Path(Cow::Borrowed("setup.cfg")),
    CacheKey::Directory {
        dir: Cow::Borrowed("src"),
    },
]
```

### `GitPattern` / `GitSet`

```rust
// from crates/uv-cache-info/src/cache_info.rs
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged, rename_all = "kebab-case", deny_unknown_fields)]
pub enum GitPattern {
    Bool(bool),
    Set(GitSet),
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct GitSet {
    commit: Option<bool>,
    tags: Option<bool>,
}
```

兩種寫法：

- `{ git = true }` / `{ git = false }` — 等同於 `{ commit = true, tags = false }` / 全關。
- `{ git = { commit = true, tags = true } }` — 兩個維度分別控制。

## 命名警示

`uv_cache_info::CacheKey` 這個 enum 跟 [[cache-key-trait]] 裡的 `uv_cache_key::CacheKey` trait 完全是兩回事。後者是 hashing 介面，前者是 cache invalidation 設定。他們唯一的共同點是「用來標識什麼東西需要被 cache 區分」這個語義靈感。實務上用全 path import (`uv_cache_info::CacheKey` vs `uv_cache_key::CacheKey`) 來區分，更詳細的對比見 [[cache-info-vs-cache-key]]。
