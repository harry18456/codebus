## 1. Cargo Workspace 結構

- [x] 1.1 root `Cargo.toml` 改成 cargo workspace（`[workspace]` + members `codebus-core` / `codebus-cli` / `codebus-app`）+ shared `[workspace.package]`（version / edition / license / authors）+ shared `[workspace.dependencies]`（先放 `clap` 預備 #2.1 用）
- [x] 1.2 `codebus-core/Cargo.toml` + `codebus-core/src/lib.rs` 空殼（lib.rs 內含一個 `pub fn placeholder() {}` 避免 cargo 對空 lib 的警告；description 寫 "Core library for codebus: vault primitives, wiki lint, schema content"）
- [x] 1.3 `codebus-app/Cargo.toml` + `codebus-app/src/main.rs` + `codebus-app/src/lib.rs` 純 placeholder（main.rs 印 `codebus-app placeholder; not yet implemented` 並 exit 0；lib.rs 空 `pub fn placeholder() {}`；Cargo.toml description 寫 "Tauri desktop app for codebus (placeholder for future implementation)"，**不引入 tauri / wry / 任何 GUI 依賴**）

## 2. CLI Shell

- [x] 2.1 `codebus-cli/Cargo.toml` + `codebus-cli/src/main.rs`：用 clap derive 定義 `Cli` struct + `Command` enum 落實 Subcommand Registration（`init` / `goal` / `query` / `lint` / `fix` 五 verb）+ No-Arg Defaults to Init Dispatch（`#[command(subcommand)]` 用 `Option<Command>`，`None` → 走 init handler 的 dispatch path 與 explicit init 一致）；main 用 `tokio::main`（async runtime 給後續 verb 用）；exit code 從 handler 的 `ExitCode` 回傳
- [x] 2.2 `codebus-cli/src/commands/{mod.rs,init.rs,goal.rs,query.rs,lint.rs,fix.rs}`：各 verb 一個檔，落實 Stub Verb Exit Behavior（各 handler 印 `<verb>: not yet implemented` 至 stderr 並回 `ExitCode::from(1)`；不 panic、不阻塞、不 silently no-op）；`mod.rs` 公開 5 個 handler 給 main 的 dispatch match 用

## 3. Verify

- [x] 3.1 寫 `codebus-cli/tests/cli_routing.rs` 整合測試（用 `env!("CARGO_BIN_EXE_codebus")` 跑 binary 子 process），覆蓋三條 Requirement 各自全部 Scenario：Subcommand Registration（`--help` 列 5 verb / `--version` 印 version / unknown verb 被 clap reject 退出非零）；No-Arg Defaults to Init Dispatch（bare `codebus` vs `codebus init` 同 stderr / 同 exit code）；Stub Verb Exit Behavior（5 verb 各自 stderr 含 `not yet implemented` + 退出非零）
- [x] 3.2 跑 `cargo build` 整 workspace clean（無 warning 阻擋級錯誤，3 個 crate 都成功 build）+ `cargo test` 全綠
