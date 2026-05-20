## Context

codebus 在 `<vault>/.codebus/.claude/settings.json` 寫 PreToolUse hook 攔截 agent 工具呼叫。目前只有一條 Bash matcher（指向 `codebus hook check-bash`，存在於 `Fix Bash Hook Installation` requirement），其他工具完全不擋。

Claude Code 的 Read tool 可讀取常見圖片格式（PNG / JPG / PDF / GIF / WebP / BMP / TIFF / HEIC / AVIF 等），把 binary 內容當 visual context 直接餵給 model。codebus 的 PII 防線在 `vault::raw_sync` 階段只跑 regex_basic 對**文字內容**做 redaction，圖片走 Read 完全 bypass 這層。

威脅模型：repo 內含 screenshot（credentials UI / 內網 dashboard / 個人臉孔 / 第三方授權頁面）時，agent 可在 goal / query / chat 任一 verb 期間 Read 圖片，並把對圖片內容的觀察寫進 wiki。一旦寫進 `.codebus/wiki/`，nested git auto-commit 會把它打包；之後 user 提交也會包含。違反 codebus 「PII-sanitized wiki」核心保證。

既有架構 anchor：
- Hook installer：`codebus-core/src/vault/settings.rs` 的 `DEFAULT_SETTINGS_JSON` 常數
- Hook implementer：`codebus-cli/src/commands/hook.rs` 的 `HookArgs::CheckBash` variant + `check_bash` 函數 + 工具函數（`emit_block` / `json_escape` / `PreToolUseInput` struct / `is_codebus_binary`）
- Spec normative source：`openspec/specs/lint-feedback-loop/spec.md` 的 `Fix Bash Hook Installation` requirement

## Goals / Non-Goals

**Goals:**

- 封住「圖片走 Read → bypass PII filter → 寫進 wiki」這條洩漏路徑
- 對既有 hook pattern 保持結構對稱（hook.rs 同樣 enum-driven、settings.rs 同樣 write-if-missing）
- Cross-platform 一致行為（Windows / macOS / Linux 都套同樣副檔名比對規則）
- 既有測試契約（write-if-missing 對既有 settings.json byte-identical 不覆寫）不被破壞

**Non-Goals:**

- magic bytes / mime sniff 偵測（trust 副檔名）
- 白名單反向：只允許 .md / .rs / .py / ... 等文字檔
- SVG 內嵌 base64 image 偵測
- 對既有已 init 過的 vault 自動 retro-fit 補 Read hook
- 加 `codebus init --migrate-hooks` 子旗標進行 settings.json diff 合併
- audio / video 副檔名（mp3 / wav / mp4 / mov 等）
- user-configurable 副檔名 allowlist / blocklist（先 hardcoded，等 case 出現再考慮）

## Decisions

### ASCII case-insensitive 跨平台一致（偏離 is_codebus_binary 的 OS-split）

副檔名比較走 ASCII case-insensitive，**所有 OS 都套同樣規則**。

既有 `is_codebus_binary`（`hook.rs:124-140`）在 Windows 走 case-insensitive、在 Unix 走 case-sensitive，理由是 binary basename 在 POSIX 是 case-sensitive。**本 change 刻意偏離這個 pattern**：副檔名語意上不分大小寫（Linux 上 `screenshot.PNG` 跟 `screenshot.png` 都是 PNG 圖片），若沿用 OS-split 行為，Linux user 用 `.PNG` 大寫副檔名會繞過攔截，PII 仍洩漏。

Alternatives considered：
- 沿用既有 OS-split → 行為 leak，rejected
- 統一 case-sensitive → 影響 Windows 上常見大寫副檔名 case，rejected

### Blacklist 副檔名清單（不採 whitelist / magic bytes）

Block 固定清單：`png` / `jpg` / `jpeg` / `gif` / `webp` / `bmp` / `tiff` / `tif` / `pdf` / `ico` / `heic` / `heif` / `avif`。SVG 不擋（XML 純文字、可走 regex_basic 文字掃描）。

Alternatives considered：
- Magic bytes / mime sniff：副檔名在 codebus 環境下可信（user 自己 vault），加開檔讀 header 是 over-engineering，且增加 hook latency
- 白名單反向：副檔名宇宙太雜（md / rs / py / ts / json / yaml / toml / sh / ps1 / go / java / rb / php / cs / kt / swift / dart / vue / svelte / ...），維護成本永久增加，且新語言 / 新工具新副檔名會被誤擋

### 新增 `check-read` subcommand 鏡射 `check-bash`

`HookArgs` enum 加 `CheckRead` variant，新 `check_read` 函數複用 `PreToolUseInput` struct（讀 `tool_input.file_path` 而非 `tool_input.command`）、`emit_block`、`json_escape`、fail-closed default。

Alternatives considered：
- 擴 `check_bash` 為更通用的 dispatcher（讀 `tool_name` 分流）：破壞既有測試 layout、增加單一函數職責，rejected
- 拆獨立 module：scope creep，rejected

### 既有 vault 升級走 release note（不自動 migrate）

`write_settings_if_missing` 保持 write-if-missing byte-identical 語意。已 init 的 vault 升級**不**自動補第二條 hook —— release note 列 JSON snippet 引導 user 手動加，或 re-init 在新 location。

Alternatives considered：
- 自動 migrate：破壞既有 `does_not_overwrite_existing_settings_json` 測試契約（`settings.rs:97-108`），且需引入 JSON diff 合併邏輯（破壞 user 客製化 hooks 的風險），rejected
- 加 `--migrate-hooks` flag：scope creep（需設計 conflict 處理），延後 backlog

### Fail-closed 沿用 check-bash 模式

stdin 空 / 非 JSON / 缺 `tool_input.file_path` / 非 string / 空 string → emit_block。任何 hook 故障一律 block 圖片，**不**讓 silent allow 偷渡。

Alternatives considered：
- Fail-open：違反 PII safety floor 原則，rejected

## Implementation Contract

**Behavior:**

當 agent 在 vault context 內呼叫 Read tool 讀取黑名單副檔名的檔案時，PreToolUse hook 攔截並回傳 block decision JSON，Claude Code 對 user 顯示 block reason，agent 看到 reason 後得知該檔案被禁止讀取。當副檔名不在黑名單（含 `.md` / `.rs` / `.svg` 等所有非圖片副檔名）時，hook 不出聲、Read tool 正常執行。

**Interface / data shape:**

- CLI subcommand：`codebus hook check-read`（internal、hidden from --help，與 check-bash 並列）
- Stdin：PreToolUse JSON 物件 `{"tool_name":"Read","tool_input":{"file_path":"<absolute or relative path>"}}`（未知欄位忽略）
- Stdout（block）：JSON 物件 `{"decision":"block","reason":"<msg>"}`，message 內含「禁止讀取的副檔名」訊息與檔名
- Stdout（allow）：空（exit 0、無 JSON）
- Exit code：永遠 0（block 也用 0，reason 走 stdout）

settings.json 新形狀（vault init 時寫入）：

```
{
  "hooks": {
    "PreToolUse": [
      { "matcher": "Bash", "hooks": [{ "type": "command", "command": "codebus hook check-bash" }] },
      { "matcher": "Read", "hooks": [{ "type": "command", "command": "codebus hook check-read" }] }
    ]
  }
}
```

**Failure modes:**

- stdin 讀取失敗 → block，reason `"hook: failed to read stdin"`
- stdin 空 → block，reason `"hook: empty stdin (no PreToolUse JSON received)"`
- 非 JSON → block，reason `"hook: malformed PreToolUse JSON on stdin"`
- 缺 `tool_input.file_path` 或非 string → block，reason `"hook: tool_input.file_path absent or empty"`
- 副檔名命中黑名單 → block，reason `"hook: reading image / binary files is blocked to prevent PII bypass; received <path>"`

**Acceptance criteria:**

- hook.rs unit tests：黑名單副檔名命中（含混合大小寫 `Foo.PNG` / `bar.Jpeg`）、白名單副檔名放行（`.md` / `.rs` / `.svg` / `.txt`）、Windows 反斜線 + Unix 正斜線路徑分隔符都正確抽副檔名、缺 file_path / 非 string / 空 string fail-closed
- settings.rs integration test：fresh vault init 後 settings.json 含兩條 PreToolUse entry（Bash 跟 Read），都通過 JSON schema 驗證
- CLI integration test（`codebus-cli/tests/`）：vault init → 手構 PreToolUse JSON 餵給 `codebus hook check-read` stdin → assert block 對黑名單、allow 對白名單
- Manual smoke（Windows）：cargo tauri dev → 一個含 screenshot.png 的 repo → 跑 goal verb → 觀察 agent 嘗試 Read 該圖檔被 block reason 阻擋

**Scope boundaries:**

In scope：黑名單副檔名常數、`check_read` 函數、`HookArgs::CheckRead` variant、`is_image_path` predicate、settings.json template 第二條 entry、unit tests、settings integration test、CLI integration test、release note 升級指引段落。

Out of scope：magic bytes / mime sniff、白名單反向、SVG 內嵌 image 偵測、自動 migrate 既有 vault、user-configurable allowlist、audio / video 副檔名、`Glob` / `Grep` / `Edit` / `Write` 的 PreToolUse hook（本 change 只處理 Read）。

## Risks / Trade-offs

- [既有 vault 沒升級仍暴露] → Mitigation: release note 列 JSON snippet + 提示 re-init option（user 自行決定）；backlog 留 `--migrate-hooks` 作為日後補強
- [新圖片副檔名（未來新格式）漏網] → Mitigation: 副檔名清單集中在 const，未來補表加副檔名 = 一行 diff；架構不需動
- [合理場景需 Read 圖片被誤擋] → Mitigation: 先擋掉、等真實 case 出現再加 user-configurable allowlist；目前 codebus 是 wiki + source code 工具，不該需要 Read 圖片
- [副檔名比較偏離 is_codebus_binary OS-split pattern 造成混淆] → Mitigation: 在 `is_image_path` 函數 doc comment 註明刻意偏離 + 連結本 change，未來 reviewer 不會誤改回 OS-split

## Observed Behavior Note (post-apply, 2026-05-20)

Task 5.1 smoke 期間實測 `codebus goal "..."` 與 `codebus query "..."` 嘗試讓 agent Read 一張 `screenshot.png`，兩個 verb 的 SKILL.md（codebus-goal / codebus-query）都在 **prompt layer** 直接拒絕 image-Read 意圖，agent 連 Read tool 都沒呼叫。代表在當前 codebus 標準 verb 流程下：

- **主要 gate**：SKILL.md 的「讀取範圍僅限 raw/code/」prompt scope
- **Backstop**：本 change 的 PreToolUse Read hook —— 當未來有 adversarial skill / prompt injection 突破 prompt scope，或 user 自己改 skill 放寬限制，hook 是 binary-layer 不可繞過的最後一道防線

這是 defense-in-depth working as designed。但代價是 **hook 在 CLI 與 GUI spawn 路徑下無法直接觀察到觸發**（因為觸發前已被 prompt layer 攔截）。Hook 的契約完整性靠 5 層證據驗證：

1. `is_image_path` predicate unit tests（hook.rs，10 條）
2. `check_read_inner` decision function unit tests（hook.rs，16 條：fail-closed + image block + allow）
3. Subprocess stdin contract integration tests（`codebus-cli/tests/hook_check_read.rs`，14 條）
4. `settings.rs` fresh-vault write asserts both Bash + Read entries（5 條）
5. 直接從命令列 `codebus hook check-read < <(echo '<json>')` 對 image / text 行為正確

未來如果有「raw mirror 規則改變、agent 真的會 Read 到圖片」的場景出現，這條 hook 才會在實機觀察到觸發；在那之前它扮演 backstop 角色。Reviewer 看 hook code 時不要誤以為「沒觀察到 = 沒用」—— 它的價值在於**當 prompt layer 失守時**仍能擋住。
