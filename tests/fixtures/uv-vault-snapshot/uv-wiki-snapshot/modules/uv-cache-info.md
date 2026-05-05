---
title: uv-cache-info crate
type: module
sources:
  - path: crates/uv-cache-info/Cargo.toml
    sha256: 971e2e145b6122d17ae2e8732d9a2bafa99eb7c7c9187669e911aabdbb040d7d
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-cache-info/src/lib.rs
    sha256: 07f78d2c256e5400211fed13192013266fef87689efa54009e81a1cf5dcf84c6
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-cache-info/src/cache_info.rs
    sha256: b877f39ea29ce5bf442af8c95be641336a8dce3ee29dcc82faadac87c3d18880
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-cache-info/src/timestamp.rs
    sha256: e73592bd2b8b13fcd3eade1a9f591c6646e07d3febe3aa39dd4d996ab429ae22
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-cache-info/src/git_info.rs
    sha256: 311af4a0db54b2cd2524354322a2f23be40640095a893f96a2e4d0a0df3a99e2
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-cache-info/src/glob.rs
    sha256: 945a2d73a51b395da057e92e4372735a8df6c617bf22072c7489a8172b725590
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
goals:
  - 了解 uv-cache-info 怎麼跟 uv-cache-key 互動
created: '2026-05-05'
updated: '2026-05-05'
related:
  - '[[cache-info]]'
  - '[[cache-info-vs-cache-key]]'
  - '[[uv-cache-key]]'
stale: false
---

# uv-cache-info crate

`uv-cache-info` 是 uv 的內部 crate，負責回答一個非常具體的問題：**這個 source distribution（來源目錄或來源檔）相對於上次建構，有沒有變？** 它把答案蒐集成一個 `CacheInfo` 物件，存在 cache 旁邊，下次要用 cache 時就把當前的 `CacheInfo` 重新算一次跟存檔比對。

## 對外 API 表面

`lib.rs` 直接 re-export 兩個子模組的全部 public 項目：

```rust
// from crates/uv-cache-info/src/lib.rs
pub use crate::cache_info::*;
pub use crate::timestamp::*;

mod cache_info;
mod git_info;
mod glob;
mod timestamp;
```

對外可見的型別：

| 名稱 | 種類 | 角色 |
|------|------|------|
| [`CacheInfo`] | struct | 蒐集到的「上次建構時的環境快照」，詳見 [[cache-info]] |
| [`CacheInfoError`] | enum | `from_path` / `from_directory` 可能的錯誤（`Glob`、`Io`） |
| [`CacheKey`] | enum | `pyproject.toml` 裡 `[tool.uv].cache-keys` 的設定條目，**不是** [[cache-key-trait]] 的那個 trait，詳見 [[cache-info]] |
| [`GitPattern`] / `GitSet` | enum / struct | `cache-keys` 裡 `{ git = ... }` 的兩種寫法（`bool` 或 `{ commit, tags }`）|
| [`FilePattern`] | enum | `Glob(String)` / `Path(PathBuf)`，目前 crate 內部沒有實際使用，但 public 暴露 |
| [`Timestamp`] | struct | 包裝 `SystemTime`，Unix 用 `ctime`、其他平台用 `mtime` |

`Commit` / `Tags` / `GitInfoError` 是 `pub(crate)`，只在 `CacheInfo` 內部使用。

## 依賴

```toml
# from crates/uv-cache-info/Cargo.toml
[dependencies]
uv-fs = { workspace = true }

fs-err = { workspace = true }
globwalk = { workspace = true }
schemars = { workspace = true, optional = true }
serde = { workspace = true, features = ["derive"] }
thiserror = { workspace = true }
toml = { workspace = true }
tracing = { workspace = true }
walkdir = { workspace = true }
```

**關鍵觀察：dependency 列表裡沒有 `uv-cache-key`。** 兩個 crate 在 build graph 上完全平行，沒有任何符號互通。它們在概念上的分工跟撞名陷阱見 [[cache-info-vs-cache-key]]。

其他依賴用途：

- **`uv-fs`**：`Simplified` trait 用來把 `PathBuf` 顯示成使用者友善格式（`debug!` 訊息用）。
- **`fs-err`**：包裝過的 IO，錯誤訊息會帶路徑。`from_path` / `from_directory` / `from_file` 全程用它。
- **`globwalk`**：`cache-keys` 支援 glob (`**/*.toml` 之類)，由 `globwalk::GlobWalkerBuilder` 實際展開。
- **`walkdir`**：`Tags::from_repository` 用它走訪 `.git/refs/tags`。
- **`toml`**：解析 `pyproject.toml` 提取 `[tool.uv].cache-keys`。
- **`schemars` (optional)**：`CacheKey` / `GitPattern` / `GitSet` 在 `feature = "schemars"` 下會 derive `JsonSchema`，給 uv 的設定 schema 文件用。

## 內部結構

| 檔案 | 角色 |
|------|------|
| `cache_info.rs` | `CacheInfo` 主體；`from_directory` / `from_file` / `from_path` 三個進入點；`CacheKey` 設定 enum；`PyProjectToml` 反序列化結構；`DirectoryTimestamp`（`Timestamp` 或 inode） |
| `timestamp.rs` | `Timestamp` 型別；Unix `ctime` vs 其他平台 `mtime` 的選擇邏輯（背後動機：`ctime` 比 `mtime` 更難被人為作假，[restic#2179](https://github.com/restic/restic/issues/2179)） |
| `git_info.rs` | `Commit` / `Tags`，從 `.git/HEAD` + `.git/refs` 讀取 commit hash 跟 tag→commit 對應；支援 worktree |
| `glob.rs` | `cluster_globs`：把多個 glob pattern 用 trie 合併成 `(longest_common_base, [patterns])`，目的是同一個目錄只被 walk 一次 |

`Cargo.toml` 設 `doctest = false`，所以文件區塊的範例不會被 cargo test 跑到。

## 進入點：`CacheInfo::from_directory`

絕大多數 `uv-cache-info` 的工作集中在 `from_directory`：

1. 讀 `<dir>/pyproject.toml`，嘗試解析 `[tool.uv].cache-keys`。解析失敗或不存在時，使用預設清單：`pyproject.toml`、`setup.py`、`setup.cfg`、`src/` 目錄。
2. 對每個 `CacheKey` enum variant 分流處理：
   - `Path(p)` / `File { file: p }` — 若 `p` 含 glob 符號 (`* ? [ {`) 推到 globs 清單，否則直接讀 metadata 取 timestamp。
   - `Directory { dir }` — 取目錄的 creation time（沒有的話 Unix fallback 到 inode）。
   - `Git { git: true }` — 呼叫 `Commit::from_repository`。
   - `Git { git: { commit, tags } }` — 分別決定要讀 commit 還是 tags。
   - `Environment { env }` — 讀環境變數值。
3. 如果有 globs，先用 `cluster_globs` 把它們依 longest-common-prefix 分組，再對每組跑一次 `globwalk::GlobWalkerBuilder` 收集檔案 timestamp。
4. 最後把所有 timestamp 取 max（最近修改的那個）填進 `CacheInfo.timestamp`。

`from_file` 是退化版本：直接對單一檔案取 metadata，只填 `timestamp`，其他欄位空。

`from_timestamp` 是建構子，給「已經算好 timestamp、其他不需要的場景」用，例如 [[uv-cache-key|uv-cache]] 的 `Timestamp` cache 比對。

## 下游怎麼用

`CacheInfo` 主要由 `uv-distribution` / `uv-distribution-types` 兩邊使用：

- `uv-distribution-types` 在 `CachedDist` 跟 `InstalledDist` 上都有 `cache_info: CacheInfo` 欄位，當 cache 元數據隨建構結果一起存起來。
- `uv-distribution` 的 `built_wheel_index.rs` 在拿 cache 時會：

```rust
// from crates/uv-distribution/src/index/built_wheel_index.rs
let cache_info = CacheInfo::from_directory(&source_dist.install_path)?;
if cache_info != *pointer.cache_info() {
    return Ok(None);
}
```

這裡用 `!=`（`PartialEq`）而不是 hash —— `CacheInfo` 確實 derive 了 `Hash`，但 cache 失效偵測是用結構等價判斷，不是用 hash digest。
