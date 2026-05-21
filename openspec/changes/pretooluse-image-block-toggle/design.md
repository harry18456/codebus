## Context

`pretooluse-image-block` change（2026-05-20 archive，spec `lint-feedback-loop` Requirement `PII Image Read Hook Installation`）將 `codebus init` 預設寫入的 `.codebus/.claude/settings.json` 加上第二條 PreToolUse hook（Read matcher → `codebus hook check-read`），擋 PNG/JPG/PDF 等圖片 / binary 副檔名避免 PII bypass。Hook 行為是 **hard-coded blocklist + fail-closed**，user 沒任何 escape hatch。

實際使用上有三個摩擦：

1. **沒有 user-facing 開關**：user 想暫時讓 agent Read 一張 screenshot（debug / 對照 / 證據蒐集）必須手改 `.codebus/.claude/settings.json` 移除 Read entry，且每個 vault 各改一次
2. **migration 不對稱**：新 init 的 vault 帶兩條 hook，但 `pretooluse-image-block` 之前 init 過的 vault 只有 Bash hook（由 `write_settings_if_missing` 的 byte-identical 契約決定）。如果未來想「全部 vault 都關掉 image block」就要逐個 edit
3. **Settings UI 沒入口**：user 不知道有這個 hook 存在、不知道現況是擋還是不擋

本 change 加 `~/.codebus/config.yaml` 的 `hooks.read_image_block` 布林 knob（預設 true）+ Settings UI 對應 toggle，讓 `codebus hook check-read` 在每次 invocation 開頭讀 config 決定 block / allow。Config 是 single source of truth，立刻對所有既有 vault 生效（不管 vault 的 settings.json 有沒有 Read entry，hook subcommand 是同一支 binary）。

既有架構 anchor：

- Hook installer：`codebus-core/src/vault/settings.rs` 的 `DEFAULT_SETTINGS_JSON` 常數（pretooluse-image-block 改過）
- Hook implementer：`codebus-cli/src/commands/hook.rs` 的 `check_read` 函數 + `check_read_inner` 純函數 + `IMAGE_BLOCKLIST` 常數
- 既有 config 區塊 pattern：`codebus-core/src/config/{pii,lint,quiz,goal,log}.rs` 都是獨立檔，loader 模式一致（`load_X_config(path) -> Result<XConfig, ConfigLoadError>`）
- STARTER_CONFIG：`codebus-core/src/config/global_starter.rs` 含註解齊全的預設 yaml
- Settings UI：`codebus-app/src/components/settings/SettingsModal.tsx` 有既有 toggle row pattern（`lint.fix.enabled` / `quiz.content_verify` / `goal.content_verify` / `log.sink: none`）

## Goals / Non-Goals

**Goals:**

- User 可透過編輯 `~/.codebus/config.yaml` 或 App Settings UI 開關 Read hook，**全 vault 立刻生效**（不需 re-init / 不需逐 vault 改檔）
- 預設 ON 保留 PII safety floor 行為（user opt-out 模型，跟既有 pretouseluse-image-block 預設一致）
- 既有 vault config.yaml 沒 hooks section → tolerant default true → 行為不變，**不需 migration doc**
- Settings UI 顯示當前狀態 + 警告文案（關掉等於 PII safety floor 失效）
- 不破壞既有 `codebus hook check-read` 對 stdin malformed / 副檔名命中 等行為（fail-closed 仍是 fail-closed）

**Non-Goals:**

- 拆 Bash hook toggle（pretooluse-image-block 設計就是 Bash hook 是 fix 沙箱必要 gate）
- per-vault override（YAGNI；想客製化的 user 手動編 vault settings.json）
- 整個 hook system 全部 toggleable（scope creep）
- 「擋 / 允許某個特定副檔名」精細控制（YAGNI；想客製化的 user 編 vault settings.json）
- `codebus config get/set` 加新子命令支援 hooks 區塊（既有 config CLI 只管 keyring key）
- 觀察性 logging：hook 被 disable 不額外寫 event（events.jsonl 已有間接觀察性）

## Decisions

### Config key: `hooks.read_image_block: true`（top-level `hooks` section，預設 true）

Yaml 形狀：

```
hooks:
  read_image_block: true
```

Alternatives considered：

- **`pii.read_image_block`** 放 pii section：語義上跟「PII safety floor」相關但**位置 wrong** —— pii section 管的是 `regex_basic` scanner 對 raw mirror 的掃描行為，這條是 **hook gate**。`hooks.` namespace 更乾淨且預留未來其他 hook toggle 的空間 —— rejected
- **`agent.image_read_block`** 放新 agent section：agent section 不存在、為了一個 knob 開新 section overkill —— rejected
- **`vault.settings_json.read_image_block`** 暗示 vault-internal：但 knob 是 global 不是 per-vault，命名誤導 —— rejected

新增 top-level `hooks` section 是最自然 namespace。命名 `read_image_block` 而非 `read_hook_enabled` 是因為未來可能有「擋 audio」「擋 PDF only」等變化，`read_image_block` 表達意圖比 generic 「read hook enabled」明確；若未來加更多 hook toggles 都進 `hooks.*` namespace。

### 預設 true（opt-out），absent → true

`Default` impl for `HooksConfig` 設 `read_image_block: true`。Yaml 缺整個 `hooks` section → 沿用 default → behavior 等同 pretooluse-image-block ship 的現況，**完全無 migration cost**。

Alternatives considered：

- **預設 false（opt-in）**：等於 silent 把 PII safety floor 拿掉，違反 pretouseluse-image-block 設計動機 —— rejected
- **Required（fail-loud absent）**：跟 verify 的 required 邏輯不一樣——verify required 是因為「user 必須意識到 verify model 的 cost 變化」；image-block toggle absent → 沿用既有行為，沒 cost / 行為變化 surprise，required 純粹給 user 找麻煩 —— rejected

這跟 `verify-stage-independent-model` 的 required 設計**故意相反**：那條是「user 必須意識到 verify cost 變化」；這條是「user absent 等同既有行為」。兩條 design philosophy 都是 fail-loud-on-config-parse-error 的展現，差別在 user surprise 風險。

### Runtime check（hook subcommand 內讀 config），不在 install time gate

`codebus hook check-read` 在 `check_read` 函數開頭讀 `~/.codebus/config.yaml` 的 hooks section。若 `read_image_block: false` 立刻 exit 0 + 空 stdout（allow），不執行 stdin 解析、副檔名匹配等下游邏輯。

Alternatives considered：

- **Install-time gate（`codebus init` 寫不同 settings.json）**：要改 user 的 vault `.claude/settings.json` 才能反映 config，這違背「config 是 single source of truth」設計，且既有 vault 的 settings.json 是 if-missing 不覆寫的，install-time gate 等於沒效果 —— rejected
- **Per-spawn gate（在 verb library 層讀 config 決定是否 spawn 帶 Read hook）**：但 hook 是 Claude Code 讀 `<vault>/.claude/settings.json` 決定的，codebus 在 spawn 層沒有控制力 —— rejected
- **設環境變數 `CODEBUS_READ_HOOK_DISABLED=1`**：env var 是 ephemeral，user 要每次設、且跟既有 config knob 風格不一致 —— rejected

Runtime check 是唯一能讓「config 改變立刻全 vault 生效」的方式。

### Hook subcommand 對 config 讀取失敗的 fallback：fail-safe to block

若 `~/.codebus/config.yaml` 不存在 / parse error / 缺 hooks section / 缺 read_image_block：**all 走 default true**（block）。設計選擇：safer is the right default。

具體實作：

- Config 檔不存在 → default HooksConfig → read_image_block=true → 繼續執行 blocklist 比對
- Yaml parse error → 不應該擋下 hook 本身，但 log warning to stderr + 走 default → block
- Hooks section absent → default → block
- read_image_block absent → default → block

Alternatives considered：

- **Config parse error 也讓 hook 走 allow path（fail-open）**：違反 pretouseluse-image-block 設計的 fail-closed philosophy —— rejected

任何 ambiguous 狀態都偏向 block 不偏向 allow。這跟既有 `check_read` 對 stdin malformed 也 fail-closed 是同一個 philosophy。

### Settings UI 位置：SettingsModal 加新 row，不在 EndpointSection 內

Toggle row 加在 SettingsModal 既有 toggle 群（`lint.fix.enabled` / `quiz.content_verify` / `goal.content_verify` / `log.sink` 之中或之後），不放進 EndpointSection。

Alternatives considered：

- **放 EndpointSection 內**：EndpointSection 管 Claude Code endpoint profile（system / azure profile / model / effort），hooks 是不同 concern；混進去 conceptually 不對 —— rejected
- **新開 "Hooks" section**：未來如果有更多 hook toggles 才有道理；現在只有一個 toggle，新 section 是 over-engineering —— rejected。預留未來：若有第 2 個 hook toggle 進來，**那時**才重構出 Hooks section

加進既有 toggle 群是最小變動且符合既有 UI 平鋪 toggle 的 information architecture。

## Implementation Contract

**Behavior:**

當 `~/.codebus/config.yaml` 含 `hooks.read_image_block: false` 時，所有 vault 下的 agent spawn 觸發 Read tool 都不被擋（hook 的副檔名 blocklist 比對被 short-circuit）。當 config 為 true / absent / parse error / hooks section 缺：hook 行為等同 pretouseluse-image-block ship 的現況（blocklist 比對 + fail-closed default）。

User 在 App Settings 開啟 modal、看到「Block image / binary reads」toggle 顯示當前狀態；toggle 後 Save → save_global_config 寫新 yaml → 下次任何 vault 的 agent spawn 立刻反映新狀態（不需要 re-init / 不需要重啟 App）。

**Interface / data shape:**

- 新 config struct（in Rust）：

  ```
  pub struct HooksConfig {
      pub read_image_block: bool,  // default true
  }
  ```

- Yaml schema：

  ```
  hooks:
    read_image_block: true   # or false
  ```

- `codebus hook check-read` 子命令的 stdin / stdout 契約**不變**（既有 PreToolUse JSON 解析 + decision JSON 輸出）；變的是內部開頭多一個 config 讀取分支
- TypeScript（`ipc.ts`）：

  ```
  export interface HooksConfig {
    read_image_block: boolean
  }
  ```

- `~/.codebus/app-state.json` 不動（hooks 設定屬於 global config 不屬於 app state）

**Failure modes:**

- Config 檔不存在 → default HooksConfig → read_image_block=true → 跑 blocklist 比對（fail-safe to block）
- Config yaml parse fail → stderr warning + 走 default → block
- Hooks section 缺 → default → block
- read_image_block 為非布林（如字串 "yes"）→ parse error → 走 default → block
- 既有 `check_read` 對 stdin 的 fail-closed 不變（empty / malformed / missing file_path / non-string → block）

**Acceptance criteria:**

- `codebus-core` 單元測試：
  - `HooksConfig::default()` 回傳 read_image_block=true
  - load_hooks_config from yaml `hooks: { read_image_block: false }` 回 false
  - load_hooks_config from yaml without hooks section → tolerant default true
  - load_hooks_config from malformed yaml 走 default + 不 panic
- `codebus-cli` 單元測試（`hook.rs`）：
  - `check_read_inner` 接受新 signature 帶 HooksConfig 參數（或 testable indirection），config.read_image_block=false → 永遠回 None（allow）；不論 stdin 是 image / 非 image / malformed
  - config.read_image_block=true → 行為跟既有 `check_read_inner` 完全一致（既有測試應全綠）
- `codebus-cli` 整合測試（`hook_check_read.rs`）：
  - 設一個 CODEBUS_HOME 指向含 `hooks.read_image_block: false` 的 yaml 檔；對 image stdin 跑 `codebus hook check-read` → stdout 空（allow）
  - 同上 yaml 設 true → 行為跟既有測試一致（blocklist 命中 block）
  - 沒 hooks section 的 yaml → 行為跟既有一致（block）
- `codebus-core` STARTER_CONFIG round-trip 測試：starter 含 hooks section 註解、round-trip 後 HooksConfig::default() 對齊
- `codebus-app` 前端測試（`SettingsModal.test.tsx`）：
  - toggle row 渲染、初始值反映 config
  - 點 toggle 改動 dirty state、Save 後 payload 含 `hooks.read_image_block`
  - 文案含「PII safety floor」相關警告字串（i18n key 對齊）

**Scope boundaries:**

In scope：

- 新 config 區塊 `hooks.read_image_block`（含 Rust struct + load_hooks_config helper + Default impl + TypeScript interface + STARTER_CONFIG 註解）
- `check_read` 子命令的 runtime config 讀取邏輯
- SettingsModal 加 toggle row
- i18n zh-tw + en
- 對應測試（單元 + 整合 + 前端）

Out of scope：

- Bash hook toggle
- per-vault override
- 整個 hook system optional
- 副檔名 blocklist 客製化
- 新 CLI config subcommand
- 觀察性 logging

## Risks / Trade-offs

- [User 不小心關掉、忘記 PII 風險] → Mitigation: Settings UI toggle 文案明確警告「關閉後 agent 可 ingest 圖片，bypass PII filter」；既有 SettingsModal pattern（`quiz.content_verify` cost 警告）已是同樣風格
- [Hook 每次 invocation 多讀一次 config（~ms 級 IO）] → Mitigation: 可忽略，hook 本來就要 spawn + JSON parse stdin + stdout，多一次 yaml read 比例極小；測試確認 hook invocation 仍 sub-100ms
- [Config 改動 → next agent spawn 才生效，user 不知道] → Mitigation: 行為合理（hook 是 agent spawn 時 Claude Code 讀的，user 預期 spawn-level reflect），Settings UI Save 完文案可加「下次 agent spawn 生效」hint；屬於 polish-level，本 change 暫不寫進 spec
- [`hooks` namespace 預留未來擴展 vs 直接用 flat `read_image_block`] → 選 `hooks.read_image_block`（nested namespace），跟既有 `pii` / `lint` / `quiz` / `goal` / `log` 風格一致；未來加更多 hook toggle 不需重構
- [既有 `check_read_inner` 是 pure function、加 config 參數打破 signature] → Mitigation: 加 `check_read_inner` 第二參數 `config: &HooksConfig`，既有測試更新傳預設 config；單元 testability 不變
