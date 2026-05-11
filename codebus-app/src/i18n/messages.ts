/**
 * Single-source i18n message bundle.
 *
 * Conventions:
 * - Flat dotted keys: `<screen>.<area>.<purpose>` (e.g. `lobby.empty.title`).
 * - `{varName}` placeholders are filled by the interpolate helper in
 *   `useT` (regex-based, see `useT.ts`).
 * - Both locales MUST share the same key set. TypeScript's `keyof
 *   typeof messages.en` is the source of truth — `messages.zh` must
 *   satisfy `Record<keyof typeof messages.en, string>` (enforced below).
 * - Add new keys here BEFORE consuming them in JSX; `useT` is typed so
 *   a missing key is a compile error.
 */

const en = {
  // ---- Common ----
  "common.cancel": "Cancel",
  "common.save": "Save",
  "common.saving": "Saving…",
  "common.dismiss": "Dismiss",
  "common.justNow": "just now",
  "common.minutesAgo": "{n}m ago",
  "common.hoursAgo": "{n}h ago",
  "common.daysAgo": "{n}d ago",
  "common.appName": "codebus",

  // ---- Lobby topbar ----
  "lobby.topbar.newVaultButton": "+ New Vault",
  "lobby.topbar.newVaultShortcutHint": "⌘N",

  // ---- Lobby populated state ----
  "lobby.populated.sectionLabel": "Recent vaults",
  "lobby.populated.dragTip":
    "tip · Drag a repo folder anywhere into this window to open it as a vault.",

  // ---- Lobby empty state ----
  "lobby.empty.title": "Board your first bus",
  "lobby.empty.subtitle":
    "Pick a repo, run a goal, and let codebus map the codebase with you.",
  "lobby.empty.cta": "+ Board a new bus",
  "lobby.empty.quickstartLabel": "QUICKSTART",
  "lobby.empty.step1": "Pick a repo folder",
  "lobby.empty.step2": 'Run a goal — e.g. "搞懂這 repo 的 X"',
  "lobby.empty.step3": "Quiz yourself to verify",

  // ---- Vault card ----
  "vaultCard.lastOpened": "last opened",
  "vaultCard.missingBadge": "missing",
  "vaultCard.menu.revealInFiles": "Open in file manager",
  "vaultCard.menu.remove": "Remove from list",

  // ---- Bottom strip ----
  "bottomStrip.settings": "Settings",

  // ---- Window controls (aria-labels) ----
  "windowControls.minimize": "Minimize",
  "windowControls.maximize": "Maximize",
  "windowControls.restore": "Restore",
  "windowControls.close": "Close",

  // ---- Drop target overlay (drag-over feedback) ----
  "dropTarget.title": "Drop to add vault",
  "dropTarget.subtitle": "Folder will be added to your vault list.",

  // ---- Loading overlay ----
  "loading.title": "Boarding the bus…",
  "loading.subtitle":
    "Setting up vault: copying source, scanning PII, writing wiki layout, initializing nested git. Larger repos take 3–15 seconds.",

  // ---- Detection dialog (existing .codebus/) ----
  "detection.title": "This folder already has a codebus vault",
  "detection.justBind.label": "Just bind it to Lobby (recommended)",
  "detection.justBind.help":
    "Add to the lobby without modifying any existing data.",
  "detection.reInit.label": "Re-initialize (destructive)",
  "detection.reInit.help":
    "Delete the existing .codebus/ directory and run a fresh init.",
  "detection.confirmInput.label": "Type {keyword} to confirm:",
  "detection.confirmInput.aria": "Type delete to confirm",
  "detection.confirm.justBind": "Just bind",
  "detection.confirm.reInit": "Delete & re-initialize",

  // ---- Settings modal ----
  "settings.title": "Global Settings",
  "settings.fields.aiProvider.label": "AI Provider",
  "settings.fields.aiProvider.value": "Claude CLI",
  "settings.fields.aiProvider.note": "only option for now",
  "settings.fields.auth.label": "Authentication",
  "settings.fields.auth.connected": "✓ Connected",
  "settings.fields.auth.disconnected": "Disconnected",
  "settings.fields.auth.reauthenticate": "Re-authenticate…",
  "settings.fields.defaultModel.label": "Default model",
  "settings.fields.defaultModel.sublabel": "applies to all runs",
  "settings.fields.pii.label": "PII scanner",
  "settings.fields.pii.display": "regex_basic · {count} patterns",
  "settings.fields.logSink.label": "Log sink",
  "settings.fields.logSink.change": "Change folder…",
  "settings.fields.logSink.reset": "Reset",
  "settings.fields.logSink.perVaultDefault": "Per-vault default (.codebus/log/)",
  "settings.fields.quizThreshold.label": "Quiz pass threshold",
  "settings.fields.quizThreshold.sublabel":
    "% correct to pass a quiz attempt",
  "settings.fields.quizThreshold.value": "{n}%",
  "settings.fields.quizLength.label": "Default quiz length",
  "settings.fields.quizLength.value": "{n} questions",
  "settings.footer.note": "Reads/writes ~/.codebus/config.yaml",
  "settings.toast.saved": "Saved",
  "settings.reset.label": "Reset to default",
  "settings.reset.alreadyDefault": "Already at default",

  // ---- Workspace stub ----
  "workspace.backToLobby": "← Back to Lobby",
  "workspace.coming.title": "Workspace coming in v3-app-workspace-goal",
  "workspace.coming.subtitle":
    "The vault is bound; full Workspace UI lands in the next change.",

  // ---- Error messages (toast / inline) ----
  "errors.vaultAlreadyExists": "This vault is already in your list: {path}",
  "errors.vaultNotFound": "Path no longer exists: {path}",
  "errors.invalid": "{field}: {message}",
  "errors.io": "Filesystem error: {message}",
  "errors.configParse": "Config parse error: {message}",
  "errors.internal": "{message}",
  "errors.generic": "Something went wrong",
} as const

const zh: Record<keyof typeof en, string> = {
  // ---- Common ----
  "common.cancel": "取消",
  "common.save": "儲存",
  "common.saving": "儲存中…",
  "common.dismiss": "關閉",
  "common.justNow": "剛剛",
  "common.minutesAgo": "{n} 分鐘前",
  "common.hoursAgo": "{n} 小時前",
  "common.daysAgo": "{n} 天前",
  "common.appName": "codebus",

  // ---- Lobby topbar ----
  "lobby.topbar.newVaultButton": "+ 新增 Vault",
  "lobby.topbar.newVaultShortcutHint": "⌘N",

  // ---- Lobby populated state ----
  "lobby.populated.sectionLabel": "近期 Vault",
  "lobby.populated.dragTip":
    "提示 · 把 repo 資料夾拖進這個視窗就能開成新 vault。",

  // ---- Lobby empty state ----
  "lobby.empty.title": "來搭第一台公車吧",
  "lobby.empty.subtitle":
    "選一個 repo、跑一個 goal，先讓 codebus 帶你看懂這份程式碼。",
  "lobby.empty.cta": "+ 搭一台新公車",
  "lobby.empty.quickstartLabel": "快速開始",
  "lobby.empty.step1": "選一個 repo 資料夾",
  "lobby.empty.step2": '跑一個 goal — 例如「搞懂這 repo 的 X」',
  "lobby.empty.step3": "再用 quiz 驗證自己有沒有看懂",

  // ---- Vault card ----
  "vaultCard.lastOpened": "上次開啟",
  "vaultCard.missingBadge": "找不到",
  "vaultCard.menu.revealInFiles": "在檔案總管中開啟",
  "vaultCard.menu.remove": "從清單移除",

  // ---- Bottom strip ----
  "bottomStrip.settings": "設定",

  // ---- Window controls ----
  "windowControls.minimize": "最小化",
  "windowControls.maximize": "最大化",
  "windowControls.restore": "還原",
  "windowControls.close": "關閉",

  // ---- Drop target overlay ----
  "dropTarget.title": "放開即新增 vault",
  "dropTarget.subtitle": "資料夾將被加入你的 vault 清單。",

  // ---- Loading overlay ----
  "loading.title": "公車正在發車…",
  "loading.subtitle":
    "建立 vault 中：複製 source、掃 PII、寫 wiki 結構、建巢狀 git。大型 repo 約 3–15 秒。",

  // ---- Detection dialog ----
  "detection.title": "這個資料夾已經有 codebus vault",
  "detection.justBind.label": "綁定到 Lobby（建議）",
  "detection.justBind.help": "加入 lobby，不會更動任何既有資料。",
  "detection.reInit.label": "重新初始化（破壞性）",
  "detection.reInit.help": "刪除既有的 .codebus/ 目錄並重跑 init。",
  "detection.confirmInput.label": "輸入 {keyword} 以確認：",
  "detection.confirmInput.aria": "輸入 delete 確認",
  "detection.confirm.justBind": "綁定",
  "detection.confirm.reInit": "刪除並重新初始化",

  // ---- Settings modal ----
  "settings.title": "全域設定",
  "settings.fields.aiProvider.label": "AI 提供者",
  "settings.fields.aiProvider.value": "Claude CLI",
  "settings.fields.aiProvider.note": "目前唯一選項",
  "settings.fields.auth.label": "認證",
  "settings.fields.auth.connected": "✓ 已連線",
  "settings.fields.auth.disconnected": "未連線",
  "settings.fields.auth.reauthenticate": "重新認證…",
  "settings.fields.defaultModel.label": "預設 model",
  "settings.fields.defaultModel.sublabel": "套用至所有 run",
  "settings.fields.pii.label": "PII 掃描器",
  "settings.fields.pii.display": "regex_basic · {count} 條規則",
  "settings.fields.logSink.label": "Log 路徑",
  "settings.fields.logSink.change": "更換資料夾…",
  "settings.fields.logSink.reset": "還原預設",
  "settings.fields.logSink.perVaultDefault": "各 vault 自己的 .codebus/log/",
  "settings.fields.quizThreshold.label": "Quiz 及格門檻",
  "settings.fields.quizThreshold.sublabel": "正確率達到多少算通過一次 quiz",
  "settings.fields.quizThreshold.value": "{n}%",
  "settings.fields.quizLength.label": "預設 quiz 題數",
  "settings.fields.quizLength.value": "{n} 題",
  "settings.footer.note": "讀寫 ~/.codebus/config.yaml",
  "settings.toast.saved": "已儲存",
  "settings.reset.label": "還原預設",
  "settings.reset.alreadyDefault": "目前已是預設",

  // ---- Workspace stub ----
  "workspace.backToLobby": "← 回到 Lobby",
  "workspace.coming.title": "Workspace 將在 v3-app-workspace-goal 推出",
  "workspace.coming.subtitle":
    "Vault 已綁定；完整的 Workspace UI 會在下一條 change 完成。",

  // ---- Errors ----
  "errors.vaultAlreadyExists": "這個 vault 已經在清單裡了：{path}",
  "errors.vaultNotFound": "路徑已不存在：{path}",
  "errors.invalid": "{field}：{message}",
  "errors.io": "檔案系統錯誤：{message}",
  "errors.configParse": "Config 解析錯誤：{message}",
  "errors.internal": "{message}",
  "errors.generic": "發生未預期的錯誤",
}

export const messages = { en, zh } as const
export type MessageKey = keyof typeof en
