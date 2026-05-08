## Why

V3 path D pivot 第一個 change。Roadmap (`docs/v3-roadmap.md` commit `290bb92`) §4 列出 9 個 change，本 change 是 #1：建立 cargo workspace 跟 CLI subcommand 骨架，**讓後續 8 個 change 有立足點可以加 module**。

對齊 v2 已驗證 workspace 結構（[`legacy/v2-rust/Cargo.toml`](../../../legacy/v2-rust/Cargo.toml)：3 crate workspace），但 CLI shape 改 subcommand mode（v2 是 flag mode；path D 需要 trigger 語意明確的 subcommand）。

## What Changes

- Root `Cargo.toml` 從單檔變成 cargo workspace，3 個 member：
  - `codebus-core`（lib）：之後 #2/#3/#4/#6/#8 在此加 vault / lint / config 模組。本 change 只放空殼 lib.rs
  - `codebus-cli`（bin `codebus`）：clap subcommand mode 5 verb（`init` / `goal` / `query` / `lint` / `fix`）+ no-arg → 預設 dispatch 到 `init`
  - `codebus-app`（bin `codebus-app`）：Tauri placeholder，內部 `println!` 即可。本 change 不引入 tauri crate
- 5 verb subcommand 在本 change **皆為 stub**：執行時 stderr 印 `<verb>: not yet implemented`、exit 非零。後續 change 各自改寫對應 stub
- CLI integration test 驗證：clap help 列 5 verb / `--version` 印 cargo pkg version / no-arg 走 init stub / 5 verb 各自吐 not-yet-implemented 訊息
- `cargo build` 跨整個 workspace clean

## Non-Goals

- 不引入 tauri / wry / 任何 GUI 依賴（`codebus-app` 純 placeholder）
- 不寫任何 verb 的真實邏輯（5 verb 都是 stub）
- 不寫 config 模組、vault primitives、lint engine、schema 內容（皆延後到 #2-#9）
- 不寫 CLI flag 細節（如 `--repo` / `--json` / `--no-obsidian-register`）——這些 flag 隨對應 verb 實作 change 引入
- 不引入 `~/.codebus/config.yaml` 讀取（#8 v3-config 處理）
- 不寫 `mcp` subcommand——v2-skeleton 留下的 placeholder，path D 下沒角色，徹底不引入

## Capabilities

### New Capabilities

- `cli`: codebus CLI 的 subcommand routing 表面——5 verb 註冊、no-arg default 到 init、stub verb 退出行為。後續 change 在此 capability 上 ADD requirement（init / goal / query / lint / fix 各自實際行為）

### Modified Capabilities

(none)

## Impact

- Affected specs: `cli`（new）
- Affected code:
  - New:
    - Cargo.toml
    - codebus-core/Cargo.toml
    - codebus-core/src/lib.rs
    - codebus-cli/Cargo.toml
    - codebus-cli/src/main.rs
    - codebus-cli/src/commands/mod.rs
    - codebus-cli/src/commands/init.rs
    - codebus-cli/src/commands/goal.rs
    - codebus-cli/src/commands/query.rs
    - codebus-cli/src/commands/lint.rs
    - codebus-cli/src/commands/fix.rs
    - codebus-app/Cargo.toml
    - codebus-app/src/main.rs
    - codebus-app/src/lib.rs
    - codebus-cli/tests/cli_routing.rs
  - Modified: (none — fresh start at e877adc, no carried code)
  - Removed: (none)
