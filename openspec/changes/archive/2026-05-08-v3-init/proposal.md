## Why

V3 path D 第二個 change，落實 `docs/v3-roadmap.md` §4 #2。把 #1 留下的 `init` stub 換成完整 init 流程：建立 `.codebus/` vault layout、mirror raw source、register Obsidian、寫 per-repo schema、寫 manifest、寫 3 個 skill bundle、把 `.codebus` 加進 source repo `.gitignore`。

對齊 v2 已驗證的 init 流程（[`legacy/v2-rust/codebus-cli/src/commands/init.rs`](../../../legacy/v2-rust/codebus-cli/src/commands/init.rs)），但配合 path D 砍三件加四件。

**砍**：
- `goals.jsonl`（path D 不 spawn agent，無 goal tracking 需求）
- nested `.git/` 跟 `auto_commit`（vault diff 歷史是 future change 的事）
- `output/` 資料夾（v2 phase 1 後從未實際使用）

**加**：
- `.codebus/manifest.yaml`：vault metadata + sync state（`codebus_version` / `created_at` / `repo_root` / `last_sync_at` / `source_signal`），給 future `codebus goal` 偵測 source drift 提示 user 重 init、給 future `codebus migrate` 看版本決定升級
- `<repo>/.claude/skills/codebus-{goal,query,fix}/SKILL.md` 3 個 bundle 骨架（per-project 而非 global，**Claude Code 在 `<repo>/` 開時自動 load**；最小可活內容；verb 完整 workflow 是 #4/#5/#7 的事）
- `--debug` global CLI flag：default mode 維持目前 `✓` progress 行；debug mode 在每 step 之前/之後印 `[debug]` 詳細資訊（fs 操作、選擇路徑、source signal 計算等），給 user 跟未來 verb 共用
- Pre-flight `vault::sanity_check::check_repo_is_not_vault` 在 init 前擋住「在 `.codebus/` 內呼叫 init」誤用

**Schema 不雙投遞**：per-repo `.codebus/CLAUDE.md` 是 STRUCTURE（5-folder taxonomy / frontmatter / wikilinks 規則）；per-project skill bundle SKILL.md 是 ACTION（verb workflow），透過 reference 指向 vault 內 CLAUDE.md 拿 schema rules，不重複內容。對應 roadmap §3 anti-pattern。

**INGEST 修正紀錄（2026-05-08 ingest）**：

第一次 propose 時誤把 skill bundle 寫到 `~/.claude/skills/`（global）；user 指出應該是 per-project `<repo>/.claude/skills/`，**只有 cwd=<repo> 時 Claude Code 才 load**——empirical test 已驗證 cwd-scoped discovery 行為。Manifest 原本被認為「不知道幹嘛用」，user 提出「`init` 記錄 source signal 給未來 goal 偵測 drift」這個真實 use case，於是 manifest 從「meta only」改為「meta + sync state」。同時加 `--debug` flag 統一 verbose output。

延後到後續 change：
- `~/.codebus/vaults.yaml` global registry（給 Tauri starting UI 列出所有 codebus vault；本 change 不做）
- Disk space pre-flight check（給 init 在 vault 即將塞爆磁碟時 warn/refuse；本 change 不做）

## What Changes

- `cli init` 從 #1 stub 換成真實 impl；接受 `[--repo X] [--no-obsidian-register]` flag（`--repo` default pwd，no-arg `codebus` 已從 #1 routing 進 init）
- CLI 加 `--debug` global flag（top-level Cli struct 上、`global = true`），所有 verb 都接收；本 change 在 `init` 內 wire 起來
- pre-flight `vault::sanity_check::check_repo_is_not_vault` 在 init 前擋住「在 `.codebus/` 內呼叫 init」誤用
- 建立 vault 目錄：`.codebus/{wiki/[concepts,entities,modules,processes,synthesis]/, raw/code/, log/}`
- raw_sync 走 NullScanner 模式（純 mirror，PII 是 #3 v3-pii 的事）；ignore-aware（讀 source `.gitignore`）；skip `.codebus/` / `.git/` / `.env` at repo root；max 5 MiB per file。`SyncSummary` 含 `files: usize` 跟 `bytes: u64`
- 寫 `.codebus/CLAUDE.md`（per-repo schema，**write if missing** 保 user 客製化）；內容從 `codebus-core/src/schema/neutral.md` 來，是 path-D-scrubbed 的 5-folder taxonomy / frontmatter / wikilinks / page conflict / stopping criteria（不含 v2 vendor 字眼）
- 寫 `.codebus/manifest.yaml`，含 5 個 top-level 欄位 + `source_signal` 巢狀：
  - `codebus_version`（write-once，**首次 init 寫入後 re-init 不變**）
  - `created_at`（write-once UTC ISO 8601）
  - `repo_root`（write-once 絕對路徑）
  - `last_sync_at`（**每次 init 都更新**為當下 UTC ISO 8601）
  - `source_signal`（**每次 init 都重算更新**）：
    - `git_head`：`<repo>/.git/HEAD` 的字串內容（symbolic ref 或 detached sha 都直接存原文）；非 git repo 為 `null`
    - `file_count`：raw_sync mirror 進去的檔數
    - `total_bytes`：raw_sync mirror 進去的總 bytes
- 寫 3 個 skill bundle 至 **`<repo>/.claude/skills/codebus-{goal,query,fix}/SKILL.md`**（**write if missing**）
  - 最小可活內容：frontmatter `name` + `description` + 一段「trigger when `/codebus-{verb} ...`」+ 「Read `.codebus/CLAUDE.md` in this vault for schema rules」+ 「Hard scope: read 限 `.codebus/raw/code/`，write 限 `.codebus/wiki/`，禁止讀寫 outside `.codebus/` 的檔」+ 「Frontmatter `sources[].path` 用 repo-relative 邏輯路徑，**不含** `raw/code/` 前綴」
  - skill_bundle API 從 `(home_dir: &Path)` 改為 `(repo_root: &Path)`
- 若 `<repo>/.git/` 存在，把 `.codebus/` 加進 `<repo>/.gitignore`（dedup、缺檔則建立、有 trailing newline 處理；v2 ensure_codebus_in_source_gitignore carry）
- Obsidian vault register：把 `.codebus/wiki/` 寫進 user 的 `obsidian.json`（v2 carry，**fail-soft**——Obsidian 沒裝、Obsidian 在跑、parse error 都不 fatal，stderr 印 hint 後繼續）；`--no-obsidian-register` flag 完全跳過
- Init 在 default mode 輸出 `✓` progress 行（每個 step 一行）；debug mode 額外在 step 之前/之後印 `[debug] <step>: <detail>`（fs 操作、source signal 算出來的值、obsidian config path、skill bundle 寫入路徑等）

## Non-Goals

- 不寫 nested `.git/` in `.codebus/`（vault diff 歷史等 future change）
- 不寫 `goals.jsonl` 或任何 goal tracking 檔（path D 不需要）
- 不寫 `output/` 資料夾（v2 廢棄）
- raw_sync 不掛 RegexBasicScanner（PII 是 #3 v3-pii 的事；本 change 用 NullScanner stub）
- skill bundle SKILL.md 不寫完整 verb workflow（goal 流程 #4 / query 流程 #5 / fix 流程 #7 各自補完）
- 不讀 `~/.codebus/config.yaml`（config 模組是 #8 v3-config 的事；本 change 所有預設值都 hardcoded）
- 不做 OSC 8 hyperlink 或 terminal color polish（#9 v3-render-polish 的事）
- 不做 `codebus init --reinstall-skills` / `--force-manifest` 等 re-init 覆蓋 flag（YAGNI；遇到 user 移 repo 場景再開 change）
- 不做 `~/.codebus/vaults.yaml` global registry（給 Tauri starting UI 列所有 vault 用；獨立 change）
- 不做 disk space pre-flight check（vault 大致 double 專案大小，做檢查合理但獨立 change）
- 不自動清理 user 之前 dev/test 留在 `~/.claude/skills/codebus-{goal,query,fix}/` 的 leftover skill 檔（user 自行 `rm -rf` 即可）
- 不把 `<repo>/.claude/skills/codebus-*` 加進 `<repo>/.gitignore`（user 自行決定 commit 與否）

## Capabilities

### New Capabilities

- `vault`: `.codebus/` vault primitives 的 spec 集中地——layout 建立、sanity check、raw mirror（NullScanner）、source `.gitignore` mutation、per-repo schema 檔、**manifest 檔（meta + sync state）**、Obsidian vault register。後續 #3 v3-pii 在此 capability ADD「PII redaction in raw_sync」；future change 加 nested git 等也接這
- `skill-bundles`: **`<repo>/.claude/skills/codebus-{verb}/SKILL.md` bundle 寫入規範**——bundle 結構、stub 內容格式（含 hard-scope rules 跟 path translation rule）、write-if-missing semantics。後續 #4/#5/#7 在此 capability ADD 各 verb 的完整 workflow 內容要求

### Modified Capabilities

- `cli`: `init` verb 從 v3-workspace 留下的 stub 換成真實 impl + 接 `--debug` flag。**MODIFIED Requirement「Stub Verb Exit Behavior」**——`init` 從該 requirement 移除（不再是 stub），其餘 4 verb 維持 stub。**新增 Requirement「Init Subcommand Behavior」**——init 接受 flag、走 vault + skill-bundles dispatch、輸出對應 progress 行；default vs debug 兩種 verbosity。**新增 Requirement「Debug Flag Output」**——`--debug` global flag、所有 verb 共用；本 change 只在 `init` 落實 debug 行內容、其他 verb 維持 stub 不受影響

## Impact

- Affected specs: `vault`（new + manifest sync state requirement）、`skill-bundles`（new + per-project path）、`cli`（new debug flag requirement、modified init behavior）
- Affected code:
  - New:
    - codebus-core/src/vault/mod.rs
    - codebus-core/src/vault/sanity_check.rs
    - codebus-core/src/vault/layout.rs
    - codebus-core/src/vault/raw_sync.rs
    - codebus-core/src/vault/obsidian_register.rs
    - codebus-core/src/vault/source_gitignore.rs
    - codebus-core/src/vault/manifest.rs
    - codebus-core/src/schema/mod.rs
    - codebus-core/src/schema/neutral.md
    - codebus-core/src/skill_bundle/mod.rs
    - codebus-core/tests/vault_init.rs
  - Modified:
    - codebus-core/src/lib.rs（mod 宣告 vault / schema / skill_bundle，移除 placeholder fn）
    - codebus-core/Cargo.toml（加 `ignore` / `serde` / `serde_yaml` / `chrono` / `sha2` / `dirs` 等 vault primitives 用的依賴；workspace dependencies 同步補）
    - codebus-cli/src/commands/init.rs（從 stub 換成真實 dispatch；接收 `debug: bool`，每 step 視 debug 印 `[debug]` 行）
    - codebus-cli/src/main.rs（init Command variant 加 `--repo` / `--no-obsidian-register` flag；Cli struct 加 `--debug` global flag；dispatch 把 `cli.debug` 傳進 verb handler）
    - codebus-cli/Cargo.toml（加 `codebus-core` 既有依賴的 re-export 需求 + dev-dependency `tempfile` 給 init 路由 test 用）
    - codebus-cli/tests/cli_routing.rs（init 不再是 stub，原本含 init 的 stub-verb test 改 loop 4 verb：goal/query/lint/fix；no-arg 路由 test 改用 TempDir + `--repo` 確認 bare vs explicit init 仍 identical；新增 `--debug` flag 行為 test）
  - Removed: (none — #1 留下的 `placeholder` fn 在 lib.rs 改寫掉，但檔本身不 remove)
