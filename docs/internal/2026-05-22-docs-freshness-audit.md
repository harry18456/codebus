# T10 README / docs 新鮮度稽核

**Date:** 2026-05-22
**Task:** loop T10（只讀比對）
**範圍:** 主要稽核根目錄 `README.md`(155 行) 對齊實際 CLI / provider 支援。次要候選 `docs/v3-roadmap.md`、`docs/security.md`、`docs/v3-app-roadmap.md`。

---

## 找到 2 個 README drift + 3 個次要候選

### 🟡 R1：README 指令表少了 `codebus config`

**README 位置:** `README.md:63-71`（公車怎麼開 — 指令表）
**Code 事實:** `codebus-cli/src/main.rs:124 Command::Config` 是**使用者面向**子命令(無 `hide=true`)。`Hook` 才是 hidden(`:68 #[command(hide = true, subcommand)]`)。

README 表列了 7 條(`init / goal / query / chat / quiz / lint / fix`),**漏掉 `config`**。`config` 應該對應 settings 讀寫(對齊近期 settings-config-frontend / endpoint UI 等變更),使用者沒在 README 看到就不會用 CLI 改設定。
**修法:** 表加一列 `codebus config | 🔧 ...`(視 `commands/config.rs:245` 行為決定一句話描述)。10 分鐘。

### 🟠 R2：整份 README 假設只有 claude provider,完全沒提 codex / multi-provider

**README 位置:** 多處——`:30`「先確保 Claude Code CLI 已安裝且 OAuth」、`:105` 「tool whitelist」(codex 沒這玩意)、`:132` 「`claude` CLI 已安裝且 OAuth」(需求)。
**Code 事實:** 2026-05-23 archive 已落地 `codex-backend` + `codex-settings-ui`,codex 是支援的第二 provider(可設 system 或 Azure profile)。BACKLOG 也已歸檔此條。

README 對 codex 使用者**完全靜默**:不知道支援、不知道要裝什麼 CLI、不知道在哪設定。tool whitelist 那句對 codex 是錯的(它用 `-s sandbox`,PE1 已記)。
**修法選擇:**
- A(輕,半小時): 在 `## 安裝` 後加一小節「Provider 選擇」說「預設用 claude；要用 codex/Azure OpenAI 請 `codebus config` 切換並裝 codex CLI」;`:105` sandbox 描述改成「依 provider 不同 (claude: tool whitelist + PreToolUse hook; codex: `-s` sandbox)」。
- B(中): 全面寫 multi-provider 章節 + screenshot。
推薦 A 為近期 floor。

### 🟢 R3 次要：`README.md:149` 說 codebus-app「正在烤」,實際狀態

**Code 事實:** workspace 含 `codebus-app/src-tauri`(Cargo.toml:2)、Tauri 2 + Vite + React 都齊全,但 `src-tauri/tauri.conf.json:31 bundle.active=false` → 還沒打 installer(T4 spike 已確認)。
→ 「正在烤」描述基本對(可跑開發版但無 installer);算準確不算 drift。**等 F(v3-app-polish-ship)、bundle.active=true 之後**這句該改成「正式版」。提醒未來改。

### 🟡 R4 次要候選：`docs/security.md` 多 provider 視角

README:105 指向 `docs/security.md`「完整 threat model」。**未深讀**,但既然 README 本身對 codex 沉默、PE1 又指出 hook 系列描述是 claude-only,security.md 很可能也是 claude-only 視角。建議下一輪audit。

### 🟡 R5 次要候選：`docs/v3-roadmap.md` / `docs/v3-app-roadmap.md`

README:113 指向 `v3-roadmap.md`「接下來要做啥」。考量 codex + 5 個新 backlog spike 都已產出,roadmap 可能落後。建議下一輪 audit。

## 待 harry

R1 / R2 都是文檔級小修(各 10-30 分鐘),建議**順手**清掉——尤其 R2 影響新使用者上手 codex 的體驗。R3-R5 可等下一輪。
