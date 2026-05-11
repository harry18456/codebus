## Why

codebus v3 CLI 主線 10 條 change 全 ship 後（2026-05-10），下一個產品里程碑是 `codebus-app/`（Tauri 桌面 app）。design doc `docs/2026-05-11-app-ux-flow-design.md` 完成 6-screen UX brainstorming + Claude Design handoff bundle 後，現在進實做階段。

App v1 整體工作量約 7 週，切成 4 條序列化 change（避免單一巨大 change 導致 apply 失焦、review 不可行、spec drift in-flight）：

1. **v3-app-foundation**（本 change）—— Tauri shell + IPC bridge + Lobby + Settings
2. v3-app-workspace-goal —— Vault Workspace shell + Wiki preview + Goal flow
3. v3-app-quiz-cmdk —— Quiz flow + Cmd+K query drawer
4. v3-app-polish-ship —— cross-platform builds + e2e

本 change 是骨架——後三條都依賴它定義的 IPC contract、design system foundation、vault list lifecycle。

## What Changes

- 把 placeholder `codebus-app/` Cargo crate 換成完整 Tauri v2 + React 19 + TypeScript + Vite 應用
- **設計系統 foundation**：把 Claude Design handoff bundle 的 CSS variable token 翻成 Tailwind v4 theme，shadcn/ui 初始化（dark mode only、amber 單一強調色、Linear-tight 密度、Inter + JetBrains Mono）
- **IPC bridge**：Rust 端定義 5 個 Tauri command —— `list_vaults` / `add_vault` / `remove_vault` / `load_global_config` / `save_global_config`；frontend 透過 type-safe wrapper 呼叫
- **Lobby screen**：populated + empty state 兩態（design doc §4.1）；左下齒輪入口；`+ New Vault` 按鈕；vault 卡片顯示 name / path / last opened
- **New Vault flow**：file picker / drag-and-drop folder 入口 / 偵測既有 `.codebus/` 的 Just-Bind vs Re-init dialog（§4.8）
- **Global Settings modal**：7 個欄位（AI Provider read-only / Authentication OAuth status / Default model per verb / PII scanner / Log sink / Quiz pass threshold / Default quiz length），讀寫 `~/.codebus/config.yaml`（§4.7）
- **新增 `~/.codebus/app-state.json`**：app-private 設定檔，存 vault list + display_name + last_opened；CLI 完全不讀此檔
- **Workspace stub**：點 vault 卡片後切到 placeholder「Workspace coming in v3-app-workspace-goal」畫面，serve 作為 lobby ↔ workspace 狀態切換的最小可驗證 demo

## Non-Goals (optional)

(完整 Goals / Non-Goals 列表寫在 design.md，本 change 不重複)

## Capabilities

### New Capabilities

- `app-shell`: codebus-app desktop runtime contract —— Tauri 設定（window 屬性、IPC command registry、permission allowlist）、IPC command schema 與錯誤型別、`~/.codebus/app-state.json` JSON schema 與 lifecycle、Lobby UI 行為、Global Settings UI 行為與 `~/.codebus/config.yaml` routing、design system token 命名與消費規則

### Modified Capabilities

(none)

## Impact

- Affected specs: 新建 `openspec/specs/app-shell/spec.md`
- Affected code:
  - New:
    - codebus-app/src-tauri/Cargo.toml
    - codebus-app/src-tauri/tauri.conf.json
    - codebus-app/src-tauri/src/main.rs
    - codebus-app/src-tauri/src/lib.rs
    - codebus-app/src-tauri/src/ipc/mod.rs
    - codebus-app/src-tauri/src/ipc/vault_list.rs
    - codebus-app/src-tauri/src/ipc/config.rs
    - codebus-app/src-tauri/src/state/mod.rs
    - codebus-app/src-tauri/src/state/app_state.rs
    - codebus-app/src-tauri/build.rs
    - codebus-app/src-tauri/icons/icon.png
    - codebus-app/package.json
    - codebus-app/tsconfig.json
    - codebus-app/tsconfig.node.json
    - codebus-app/vite.config.ts
    - codebus-app/tailwind.config.ts
    - codebus-app/postcss.config.cjs
    - codebus-app/index.html
    - codebus-app/src/main.tsx
    - codebus-app/src/App.tsx
    - codebus-app/src/styles/tokens.css
    - codebus-app/src/styles/globals.css
    - codebus-app/src/lib/ipc.ts
    - codebus-app/src/lib/cn.ts
    - codebus-app/src/store/vaults.ts
    - codebus-app/src/store/settings.ts
    - codebus-app/src/store/route.ts
    - codebus-app/src/components/ui/button.tsx
    - codebus-app/src/components/ui/dialog.tsx
    - codebus-app/src/components/ui/input.tsx
    - codebus-app/src/components/ui/select.tsx
    - codebus-app/src/components/ui/slider.tsx
    - codebus-app/src/components/lobby/Lobby.tsx
    - codebus-app/src/components/lobby/VaultCard.tsx
    - codebus-app/src/components/lobby/EmptyState.tsx
    - codebus-app/src/components/lobby/NewVaultFlow.tsx
    - codebus-app/src/components/settings/SettingsModal.tsx
    - codebus-app/src/components/workspace/WorkspaceStub.tsx
  - Modified:
    - Cargo.toml（workspace root —— 把 codebus-app 從 placeholder 改成真的 member，調整 dependencies）
    - codebus-app/Cargo.toml（從 placeholder 升級成 binary crate metadata；實際 Rust source 移到 src-tauri/ sub-crate per Tauri v2 convention）
  - Removed: (none)
- 不影響：codebus-cli / codebus-core 既有檔案行為（app 只透過 core public API 消費，不修改 core internals）
- Runtime 新檔案：`~/.codebus/app-state.json`（schema 由本 change `app-shell` spec 定義）
- design handoff 不會被 import 成 production code（保留為 `codebus-app/design-handoff/` 純 reference）
