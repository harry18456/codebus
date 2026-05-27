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
  "lobby.topbar.newVaultButton": "+ Add",
  "lobby.topbar.newVaultShortcutHint": "⌘N",

  // ---- Lobby populated state ----
  "lobby.populated.sectionLabel": "Recent",
  "lobby.populated.dragTip":
    "tip · Drag a code folder anywhere into this window to add it to the list.",

  // ---- Lobby empty state ----
  "lobby.empty.title": "Board your first bus",
  "lobby.empty.subtitle":
    "Pick a repo, run a goal, and let codebus map the codebase with you.",
  "lobby.empty.cta": "+ Board a new bus",
  "lobby.empty.quickstartLabel": "QUICKSTART",
  "lobby.empty.step1": "Pick a repo folder",
  "lobby.empty.step2": "Run a goal — e.g.",
  "lobby.empty.step2Example": "搞懂這 repo 的 X",
  "lobby.empty.step3": "Quiz yourself to verify",

  // ---- Vault card ----
  "vaultCard.lastOpened": "last opened",
  "vaultCard.missingBadge": "missing",
  "vaultCard.menu.openLabel": "Open vault actions",
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
  "settings.fields.piiOnHit.label": "PII on-hit policy",
  "settings.fields.piiOnHit.warn": "warn",
  "settings.fields.piiOnHit.skip": "skip",
  "settings.fields.piiOnHit.mask": "mask",
  "settings.fields.piiOnHit.criticalNote":
    "Critical-severity matches (API keys) are always masked regardless of this setting.",
  "settings.fields.piiPatterns.label": "PII extra patterns",
  "settings.fields.piiPatterns.placeholder": "regex (e.g. EMP-\\d{6})",
  "settings.fields.piiPatterns.add": "Add pattern",
  "settings.fields.piiPatterns.remove": "Remove",
  "settings.fields.piiPatterns.invalid": "Invalid regular expression",
  "settings.fields.lintFix.label": "Lint fix enabled",
  "settings.fields.lintFix.sublabel": "Run the lint-and-fix loop after a goal",
  "settings.fields.quizContentVerify.label": "Quiz content verify",
  "settings.fields.quizContentVerify.cost":
    "Enabling this adds extra verify/repair agent spawns (slower, higher token cost).",
  "settings.fields.goalContentVerify.label": "Goal content verify",
  "settings.fields.goalContentVerify.cost":
    "Enabling this adds extra verify/repair agent spawns (slower, higher token cost).",
  "settings.fields.logSink.disable": "Disable logging",
  "settings.fields.endpointChat.label": "chat",
  "settings.fields.endpointChat.inherits": "inherits query ({model} / {effort})",
  "settings.fields.endpointVerify.label": "verify",
  "settings.fields.endpointVerify.tooltip":
    "Independent model for the content-verify spawn (quiz / goal). Pick a reasoning-strong model to catch hallucinations — default opus-4-6 / high encodes the \"cheap generation + expensive verification\" pattern.",
  "settings.fields.readImageBlock.label": "Block image / binary reads",
  "settings.fields.readImageBlock.warning":
    "Disabling allows the agent to read images / PDFs / binary files into its context, bypassing the regex_basic PII filter (which only scans text). Leave on unless you know the repo has no PII risk.",

  // ---- Workspace shell ----
  "workspace.backToLobby": "← Back to Lobby",
  "workspace.tab.goals": "Goals",
  "workspace.tab.wiki": "Wiki",
  "workspace.tab.quiz": "Quiz",
  "workspace.sidebar.vaultPathHint":
    "{path}\n\nClick to open in file explorer",

  // ---- Workspace · Quiz ----
  "workspace.quiz.confirmDescription":
    "The quiz will be generated from the wiki pages below — confirm to start:",
  "workspace.quiz.revise": "Re-plan",
  "workspace.quiz.confirm": "Confirm",

  // ---- Workspace · Goals ----
  "workspace.goals.newGoalButton": "+ New goal",
  "workspace.goals.emptyHint":
    "Click + New goal to ask codebus to ingest something into the wiki",
  "workspace.goals.examplePlaceholder1": "describe the authentication flow",
  "workspace.goals.examplePlaceholder2": "summarize the data ingestion pipeline",
  "workspace.goals.examplePlaceholder3": "map the public API surface",
  "workspace.goals.headerTitle": "Goals",
  "workspace.goals.headerSubtitle":
    "List what you want to understand — codebus reads the codebase one stop at a time.",
  "workspace.goals.emptyHeroTitle": "No goals yet",
  "workspace.goals.emptyHeroSubtitle":
    "Start with one of the examples below, or write your own.",
  "workspace.goals.runningTailPending": "…",

  // ---- Workspace · New Goal modal ----
  "workspace.newGoalModal.title": "New goal",
  "workspace.newGoalModal.placeholder": "What should codebus document?",
  "workspace.newGoalModal.cancel": "Cancel",
  "workspace.newGoalModal.run": "Run",
  "workspace.newGoalModal.blockedHint":
    "Wait for current run to finish or cancel it before starting a new one.",

  // ---- Workspace · Status pill labels (Phase 3B three-state) ----
  "workspace.status.done": "Done",
  "workspace.status.interrupted": "Interrupted",
  "workspace.status.failed": "Failed",
  "workspace.status.running": "Running",

  // ---- Workspace · Run detail · Running ----
  "workspace.runDetail.backLink": "← back",
  "workspace.runDetail.runningBadge": "⏺ Running",
  "workspace.runDetail.cancelButton": "⏹ Cancel",
  "workspace.runDetail.cancellingButton": "Cancelling…",

  // ---- Workspace · Run detail · Done / Cancelled / Interrupted ----
  "workspace.runDetail.doneBadge": "✓ Done",
  "workspace.runDetail.coveredPagesLabel": "Covered pages",
  "workspace.runDetail.coveredPagesEmpty": "No wiki pages changed",
  "workspace.runDetail.lintLabel": "Lint",
  "workspace.runDetail.activitySummaryLabel": "Activity summary",
  "workspace.runDetail.toolReadLine": "{n} Read",
  "workspace.runDetail.toolGlobLine": "{n} Glob",
  "workspace.runDetail.toolGrepLine": "{n} Grep",
  "workspace.runDetail.toolWriteLine": "{n} Write",
  "workspace.runDetail.toolEditLine": "{n} Edit",
  "workspace.runDetail.toolOtherLine": "{n} {tool}",
  "workspace.runDetail.thinkingLabel": "Thinking",
  "workspace.runDetail.showThinking": "Show thinking ▼",
  "workspace.runDetail.hideThinking": "Hide thinking ▲",
  "workspace.runDetail.showDetails": "Show run details ▼",
  "workspace.runDetail.hideDetails": "Hide run details ▲",
  "workspace.runDetail.phaseGoal": "goal phase",
  "workspace.runDetail.phaseFix": "fix phase",
  "workspace.runDetail.phaseQuery": "query phase",
  "workspace.runDetail.phaseChat": "chat phase",
  "workspace.runDetail.phaseOther": "{verb} phase",
  "workspace.runDetail.phaseEmptyHint": "(no tools used)",
  "workspace.runDetail.coveredPagesPhaseEmpty": "(no wiki pages changed)",
  "workspace.runDetail.cancelledBadge": "⏹ Cancelled",
  "workspace.runDetail.cancelledWarning":
    "Wiki has uncommitted changes — not auto-committed. Review in terminal if needed.",
  "workspace.runDetail.interruptedBadge": "⚠ Interrupted",
  "workspace.runDetail.interruptedWarning":
    "App was closed before this goal finished. Wiki state may be partial — review in terminal if needed.",
  "workspace.runDetail.partialTimelineLabel": "Partial timeline",
  "workspace.runDetail.retryButton": "Retry with same goal",
  "workspace.run.headerSummary": "{durationSec}s · {totalTokens} tokens",
  "workspace.run.lintSummary": "{errors} errors · {warnings} warnings",

  // ---- Workspace · Wiki ----
  "workspace.wiki.empty":
    "No wiki pages yet — run a goal to start documenting",
  "workspace.wiki.toggleTreeAria": "Toggle Pages tree",
  "workspace.wiki.pageNotFound": "Page not found",
  "workspace.wiki.openInObsidian": "Open in Obsidian",

  // ---- Workspace · Quiz placeholder ----
  "workspace.quiz.placeholder": "Coming soon — quiz flow ships in v3-app-quiz",

  // ---- Chat (cmdk) ----
  "chat.onboarding.hintEn":
    "Ask anything about this vault. AI will suggest [Promote to goal] when it fits, or you can ask AI to promote a message to a goal.",
  "chat.onboarding.hintTw":
    "可以問任何關於此 vault 的事。AI 覺得適合時會主動建議 [Promote to goal]，你也可以主動跟 AI 講想做成 goal（例如「幫我把這段寫成 goal」）。",
  "chat.placeholder.en": "Ask anything…",
  "chat.placeholder.tw": "輸入訊息…",
  "chat.button.newChat": "+ New chat",
  "chat.button.stop": "⏹ Stop",
  "chat.button.send": "Send",
  "chat.button.promote": "Promote to goal",
  "chat.button.dismiss": "Dismiss",
  "chat.toast.startedNewChat": "Started a new chat.",
  "chat.toast.undo": "Undo",
  "chat.undoToast.heading": "🆕 New chat started",
  "chat.undoToast.countdown": "({n}s to undo)",
  "chat.error.anotherGoalRunning":
    "Another goal is running. Wait for it to finish.",
  "chat.error.promoteFailed": "Promote failed. Try again.",
  "chat.token.tooltip.cacheRead": "Cache read",
  "chat.token.tooltip.cacheCreate": "Cache create",
  "chat.token.tooltip.input": "Input",
  "chat.token.tooltip.output": "Output",
  "chat.widget.aria.openChat": "Open chat",
  "chat.widget.aria.openChatWithActiveGoalRunning":
    "Open chat — a goal is running",
  "chat.widget.aria.closeChat": "Close chat",

  // ---- Error messages (toast / inline) ----
  "errors.vaultAlreadyExists": "This vault is already in your list: {path}",
  "errors.vaultNotFound": "Path no longer exists: {path}",
  "errors.invalid": "{field}: {message}",
  "errors.io": "Filesystem error: {message}",
  "errors.configParse": "Config parse error: {message}",
  "errors.internal": "{message}",
  "errors.generic": "Something went wrong",

  // ---- Settings · Endpoint sections (Cat A sweep) ----
  "settings.endpoint.claude.heading": "Claude Code endpoint settings",
  "settings.endpoint.codex.heading": "OpenAI Codex endpoint settings",
  "settings.endpoint.activeProfileAria": "Active endpoint profile",
  "settings.endpoint.activeProfileAriaCodex": "Active codex endpoint profile",
  "settings.endpoint.profile.system": "System",
  "settings.endpoint.profile.azure": "Azure",
  "settings.endpoint.profile.systemTitle": "System Profile",
  "settings.endpoint.profile.azureTitle": "Azure Profile",
  "settings.endpoint.profile.inactiveLabel": "(inactive)",
  "settings.endpoint.field.apiKey": "API key",
  "settings.endpoint.field.effort": "effort",
  "settings.endpoint.placeholder.claudeModel": "<model, e.g. opus-4-7>",
  "settings.endpoint.placeholder.codexModel": "<model, e.g. gpt-5.5>",
  "settings.endpoint.placeholder.deploymentName": "<deployment name>",
  "settings.endpoint.placeholder.azureBaseUrlClaude":
    "https://<resource>.cognitiveservices.azure.com/anthropic",
  "settings.endpoint.placeholder.azureBaseUrlCodex":
    "https://<resource>.cognitiveservices.azure.com/openai",
  "settings.endpoint.placeholder.apiVersion": "2025-04-01-preview",
  "settings.endpoint.placeholder.codexEffort": "effort",
  "settings.endpoint.keyStatus.set": "Set",
  "settings.endpoint.keyStatus.unset": "Unset",
  "settings.endpoint.keyStatus.unknown": "—",
  "settings.endpoint.keySetNew": "Set new…",
  "settings.endpoint.keyDelete": "Delete",
  "settings.endpoint.validationSummaryHeading":
    "Endpoint configuration is incomplete:",
  "settings.endpoint.saveButtonIncompleteTitle":
    "Endpoint configuration is incomplete — fix highlighted fields",
  "settings.endpoint.validation.azureProfileRequired":
    "Azure profile is required when active=azure",
  "settings.endpoint.validation.baseUrlRequired":
    "base_url is required when active=azure",
  "settings.endpoint.validation.apiVersionRequired":
    "api_version is required when active=azure",
  "settings.endpoint.validation.keyringServiceRequired":
    "keyring_service is required when active=azure",
  "settings.endpoint.validation.deploymentNameRequired":
    "{verb} deployment name is required when active=azure",
  "settings.endpoint.validation.effortInvalid":
    "{verb} effort must be one of {allowed}",
  "settings.endpoint.validation.systemModelRequired":
    "{verb} model is required when active=system",

  // ---- Settings · jargon allow-list (Cat D) ----
  // The Cat D allow-list (i18n Bundle Coverage Policy): config YAML key
  // names, verb identifiers, and codex effort enum values stay English in
  // BOTH locales — held here for centralization, not translation.
  "settings.endpoint.field.baseUrl": "base_url",
  "settings.endpoint.field.apiVersion": "api_version",
  "settings.endpoint.field.keyringService": "keyring_service",
  "settings.endpoint.verb.goal": "goal",
  "settings.endpoint.verb.query": "query",
  "settings.endpoint.verb.fix": "fix",
  "settings.endpoint.verb.verify": "verify",
  "settings.endpoint.codex.effort.low": "low",
  "settings.endpoint.codex.effort.medium": "medium",
  "settings.endpoint.codex.effort.high": "high",
  "settings.endpoint.codex.effort.xhigh": "xhigh",

  // ---- Settings · SetKeyDialog (Cat A sweep) ----
  "settings.setKeyDialog.title": "Set Azure API key",
  "settings.setKeyDialog.inputLabel":
    "Paste the API key — it will be stored in your OS keyring and never written to ~/.codebus/config.yaml.",
  "settings.setKeyDialog.errorEmpty": "API key cannot be empty",

  // ---- Settings · CLI install status (residual sweep) ----
  "settings.cliStatus.checking": "Checking…",
  "settings.cliStatus.installed": "Installed · {version}",
  "settings.cliStatus.notInstalled": "Not installed",
  "settings.fields.pii.scannerNone": "none",

  // ---- Workspace · QuizAnswering (Cat B sweep) ----
  "workspace.quiz.answering.questionCounter": "Question {n} of {total}",
  "workspace.quiz.answering.parseEmpty":
    "Quiz could not be parsed — no well-formed questions.",
  "workspace.quiz.answering.summaryHeading": "Quiz complete",
  "workspace.quiz.answering.scoreLine":
    "Score: {correct} / {total} ({percent}%)",
  "workspace.quiz.answering.outcomePassed": "Passed (threshold {n}%)",
  "workspace.quiz.answering.outcomeFailed": "Failed (threshold {n}%)",
  "workspace.quiz.answering.verdictCorrect": "Correct",
  "workspace.quiz.answering.verdictIncorrect": "Incorrect",
  "workspace.quiz.answering.submitButton": "Submit",
  "workspace.quiz.answering.nextButton": "Next",
  "workspace.quiz.answering.finishButton": "Finish",

  // ---- Workspace · QuizReview (Cat B sweep) ----
  "workspace.quiz.review.backToHistory": "← Back to history",
  "workspace.quiz.review.redoButton": "Redo this attempt",
  "workspace.quiz.review.viewLogButton": "View generation log",
  "workspace.quiz.review.viewLogClose": "Close",
  "workspace.quiz.review.summaryLine":
    "{correct} / {total} ({percent}%) — {outcome}",
  "workspace.quiz.review.yourAnswerLine":
    "Your answer: {selected} · Correct answer: {correct}",
  "workspace.quiz.review.generationLogTitle": "Generation log",

  // ---- Workspace · QuizTab (Cat B sweep) ----
  "workspace.quiz.tab.heading": "Quiz history",
  "workspace.quiz.tab.newButton": "+ New quiz",
  "workspace.quiz.tab.emptyHint":
    "No quizzes yet — start one with + New quiz",
  "workspace.quiz.tab.startButton": "Start",
  "workspace.quiz.tab.topicPlaceholder": "What do you want to be quizzed on?",
  "workspace.quiz.tab.backToHistoryShort": "← History",
  "workspace.quiz.tab.backToHistoryFull": "← Back to history",
  "workspace.quiz.tab.planningStatus": "Planning quiz scope…",
  "workspace.quiz.tab.generatingStatus": "Generating questions…",
  "workspace.quiz.tab.noMatchPrefix": "No matching wiki pages: {reason}",
  "workspace.quiz.tab.errorPrefix": "Quiz failed: {message}",
  "workspace.quiz.tab.backButton": "Back",
  "workspace.quiz.generationLogLoadError":
    "Could not load generation log: {error}",
  "workspace.quiz.headerTitle": "Quiz",
  "workspace.quiz.headerSubtitle":
    "Test how well you understood the wiki.",

  // ---- Workspace · WikiPreview action (residual sweep) ----
  "workspace.wiki.quizMeOnThis": "Quiz me on this",

  // ---- Workspace · Run detail loading state (residual sweep) ----
  "workspace.runDetail.loading": "Loading…",

  // ---- a11y (Cat C sweep) — shared accessibility keys ----
  "a11y.dialogClose": "Close",
  "chat.widget.aria.resizeChat": "Resize chat widget",
  "chat.widget.aria.minimizeChat": "Minimize chat",
  "chat.widget.title.dragToResize": "Drag to resize",
  "chat.widget.title.minimizeShortcut": "Minimize (Cmd+K)",

  // ---- Activity stream · internal sentinel markers ----
  // QGEN1: translations for `[CODEBUS_*]` thought-block sentinel
  // markers. Keys mirror the marker name in camelCase
  // (CODEBUS_QUIZ_NO_VALIDATE → codebusQuizNoValidate). New markers
  // append here; ThoughtItem auto-suppresses unknown markers.
  "activity.marker.codebusQuizNoValidate":
    "codex sandbox cannot run quiz structure validation; skipping this step.",

  // ---- Workspace · ActivityStream banner labels ----
  // Emoji is part of the label's semantic meaning; the entire string is
  // stored as one bundle value per the i18n Bundle Coverage Policy
  // emoji-prefixed scenario (do NOT split emoji + text into two keys).
  "workspace.activity.banner.start":
    "🚌 Here comes the CodeBus, rolling into {path}...",
  "workspace.activity.banner.goal": "🎯 Goal target: {goalText}",
  "workspace.activity.banner.syncStart": "🔄 Syncing source → raw/code...",
  "workspace.activity.banner.syncDone":
    "✓ Sync done ({files} files, {mib} MiB, {elapsedMs} ms)",
  "workspace.activity.banner.piiSummary":
    "🛡 PII: {scanner}, scanned {scanned}, hits {hits}, action {action}",
  "workspace.activity.banner.lintStart": "🔍 Linting...",
  "workspace.activity.banner.lintDone":
    "✓ Lint done ({errors} errors, {warns} warnings, {elapsedMs} ms)",
  "workspace.activity.banner.commitDone": "🚏 Commit {sha7}",
  "workspace.activity.banner.done": "🎉 Complete",
  "workspace.activity.banner.hint": "💡 Hint",

  // ---- Quiz badge verdict (used by lib/quiz-parse.ts) ----
  "quiz.badge.pass": "pass",
  "quiz.badge.fail": "fail",

  // ---- Settings provider CLI field label ----
  // "CLI" is jargon (universal) — en/zh values identical.
  "settings.providerCli.fieldLabel": "{name} CLI",
  "settings.providerCli.installHint":
    "Install {name} first; then reopen Settings.",

  // ---- Settings · Language dropdown ----
  // The two non-Auto options identify the language they select; their
  // displayed strings ("中文" / "English") are deliberately identical in
  // both locales per the Cat D identifier policy. Only the field label and
  // the "Auto" option vary by locale.
  "settings.language.label": "Language",
  "settings.language.auto": "Auto (system)",
  "settings.language.zh": "中文",
  "settings.language.en": "English",

  // ---- Chat token usage header indicator ----
  // "↑" arrow + numeric value composite. Value (digits or `Nk` string) is
  // pre-formatted in JS to keep the i18n template stable across the three
  // numeric branches.
  "chat.tokens.indicator": "{value} ↑",

  // ---- Workspace · ActivityStream 2-phase cluster ----
  // Spec: app-workspace § "Activity Stream Two-Phase Cluster Rendering".
  // Cluster heading is shown both during run (expanded) and after run
  // (collapsed); the *.summary.* variants only render in terminal states
  // and embed counts derived from the cluster's events.
  "workspace.activity.cluster.reading.heading": "Reading codebase",
  "workspace.activity.cluster.writing.heading": "Writing wiki",
  "workspace.activity.cluster.expand": "Expand cluster",
  "workspace.activity.cluster.collapse": "Collapse cluster",
  "workspace.activity.cluster.summary.reading":
    "Reading codebase · {reads} reads · {shell} shell · {elapsedSeconds}s",
  "workspace.activity.cluster.summary.writing":
    "Writing wiki · {new} new · {updated} updated · {elapsedSeconds}s",

  // ---- Workspace · Quiz wizard (Phase 5.4 quiz-fullscreen-wizard-view) ----
  // Spec: app-workspace § Quiz Tab Wizard Content Header And Layout.
  // The Karpathy 5-bucket identifiers (concepts / entities / modules /
  // processes / synthesis) are Cat D — see the `bucketIdentifier.*`
  // keys below; the `bucketLabel.*` keys hold the localized header
  // prose around each identifier.
  "workspace.quiz.wizard.step1.title": "Pick a topic",
  "workspace.quiz.wizard.step1.subtitle":
    "Write what you want to be quizzed on — codebus will pick wiki pages that fit.",
  "workspace.quiz.wizard.step1.placeholder":
    "e.g. how auth works / IM adapter system / message delivery flow",
  "workspace.quiz.wizard.step1.examplePillHint":
    "Click an example to fill it in. Press Enter to submit.",
  "workspace.quiz.wizard.step2.title": "Confirm scope",
  "workspace.quiz.wizard.step2.bucketLabel.concepts": "Concepts",
  "workspace.quiz.wizard.step2.bucketLabel.entities": "Entities",
  "workspace.quiz.wizard.step2.bucketLabel.modules": "Modules",
  "workspace.quiz.wizard.step2.bucketLabel.processes": "Processes",
  "workspace.quiz.wizard.step2.bucketLabel.synthesis": "Synthesis",
  "workspace.quiz.wizard.step2.bucketIdentifier.concepts": "concepts",
  "workspace.quiz.wizard.step2.bucketIdentifier.entities": "entities",
  "workspace.quiz.wizard.step2.bucketIdentifier.modules": "modules",
  "workspace.quiz.wizard.step2.bucketIdentifier.processes": "processes",
  "workspace.quiz.wizard.step2.bucketIdentifier.synthesis": "synthesis",
  "workspace.quiz.wizard.step3.title": "Generating",
  "workspace.quiz.wizard.step3.generatingHint":
    "CodeBus is reading the wiki pages and drafting questions…",
  "workspace.quiz.wizard.step4.pendingTitle": "Quiz ready",
  "workspace.quiz.wizard.step4.reviewingTitle": "Quiz: {topic}",
  "workspace.quiz.wizard.step4.completionTitle.pass": "Passed ({percent}%)",
  "workspace.quiz.wizard.step4.completionTitle.fail":
    "Did not pass ({percent}%)",
  "workspace.quiz.wizard.action.cancel": "Cancel",
  "workspace.quiz.wizard.action.back": "Back",
  "workspace.quiz.wizard.action.next": "Next →",
  "workspace.quiz.wizard.action.start": "Start",
  "workspace.quiz.wizard.action.submit": "Submit",
  "workspace.quiz.wizard.action.retry": "Retry",
  "workspace.quiz.wizard.action.redo": "↻ Redo this attempt",
  "workspace.quiz.wizard.action.viewWrong": "Review wrong questions",
  "workspace.quiz.wizard.action.viewProcess": "View generation log",
  "workspace.quiz.wizard.header.stepLabel": "Step {n} / {total} · {name}",
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
  "lobby.topbar.newVaultButton": "+ 新增",
  "lobby.topbar.newVaultShortcutHint": "⌘N",

  // ---- Lobby populated state ----
  "lobby.populated.sectionLabel": "最近",
  "lobby.populated.dragTip":
    "提示 · 把程式碼資料夾拖進這個視窗就能加入清單。",

  // ---- Lobby empty state ----
  "lobby.empty.title": "來搭第一台公車吧",
  "lobby.empty.subtitle":
    "選一個 repo、跑一個 goal，先讓 codebus 帶你看懂這份程式碼。",
  "lobby.empty.cta": "+ 搭一台新公車",
  "lobby.empty.quickstartLabel": "快速開始",
  "lobby.empty.step1": "選一個 repo 資料夾",
  "lobby.empty.step2": "跑一個 goal — 例如",
  "lobby.empty.step2Example": "搞懂這 repo 的 X",
  "lobby.empty.step3": "再用 quiz 驗證自己有沒有看懂",

  // ---- Vault card ----
  "vaultCard.lastOpened": "上次開啟",
  "vaultCard.missingBadge": "找不到",
  "vaultCard.menu.openLabel": "開啟動作選單",
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
  "settings.fields.piiOnHit.label": "PII 命中處理",
  "settings.fields.piiOnHit.warn": "warn",
  "settings.fields.piiOnHit.skip": "skip",
  "settings.fields.piiOnHit.mask": "mask",
  "settings.fields.piiOnHit.criticalNote":
    "Critical 等級（API key 等）一律強制 mask，不受此設定影響。",
  "settings.fields.piiPatterns.label": "PII 額外規則",
  "settings.fields.piiPatterns.placeholder": "regex（例：EMP-\\d{6}）",
  "settings.fields.piiPatterns.add": "新增規則",
  "settings.fields.piiPatterns.remove": "刪除",
  "settings.fields.piiPatterns.invalid": "無效的正規表達式",
  "settings.fields.lintFix.label": "啟用 lint fix",
  "settings.fields.lintFix.sublabel": "goal 完成後跑 lint-and-fix loop",
  "settings.fields.quizContentVerify.label": "Quiz 內容驗證",
  "settings.fields.quizContentVerify.cost":
    "開啟會多花 verify/repair agent spawn（較慢、token 成本較高）。",
  "settings.fields.goalContentVerify.label": "Goal 內容驗證",
  "settings.fields.goalContentVerify.cost":
    "開啟會多花 verify/repair agent spawn（較慢、token 成本較高）。",
  "settings.fields.logSink.disable": "停用 logging",
  "settings.fields.endpointChat.label": "chat",
  "settings.fields.endpointChat.inherits": "沿用 query（{model} / {effort}）",
  "settings.fields.endpointVerify.label": "verify",
  "settings.fields.endpointVerify.tooltip":
    "Content verify spawn（quiz / goal 共用）的獨立 model；建議用 reasoning 強的 model 把關。預設 opus-4-6 / high 對應「便宜出 + 貴審」策略。",
  "settings.fields.readImageBlock.label": "擋圖片 / binary 讀取",
  "settings.fields.readImageBlock.warning":
    "關閉後 agent 可 ingest 圖片 / PDF / binary 檔到 context，bypass regex_basic PII filter（只掃文字）。確認 repo 無 PII 風險才關。",

  // ---- Workspace shell ----
  "workspace.backToLobby": "← 回到 Lobby",
  "workspace.tab.goals": "Goals",
  "workspace.tab.wiki": "Wiki",
  "workspace.tab.quiz": "Quiz",
  "workspace.sidebar.vaultPathHint": "{path}\n\n點一下用檔案總管開啟",

  // ---- Workspace · Quiz ----
  "workspace.quiz.confirmDescription":
    "將依下列 wiki 頁面出題，確認後開始生成測驗：",
  "workspace.quiz.revise": "重新規劃",
  "workspace.quiz.confirm": "確認",

  // ---- Workspace · Goals ----
  "workspace.goals.newGoalButton": "+ 新增 Goal",
  "workspace.goals.emptyHint":
    "點 + 新增 Goal 讓 codebus 把某段內容整理進 wiki",
  "workspace.goals.examplePlaceholder1": "說明認證流程",
  "workspace.goals.examplePlaceholder2": "整理資料 ingest pipeline 概要",
  "workspace.goals.examplePlaceholder3": "畫出公開 API surface",
  "workspace.goals.headerTitle": "Goals",
  "workspace.goals.headerSubtitle":
    "列出你想搞懂的事——公車一站一站讀給你看。",
  "workspace.goals.emptyHeroTitle": "還沒有任務",
  "workspace.goals.emptyHeroSubtitle":
    "從下面範例挑一個開始、或自己寫一個。",
  "workspace.goals.runningTailPending": "…",

  // ---- Workspace · New Goal modal ----
  "workspace.newGoalModal.title": "新增 Goal",
  "workspace.newGoalModal.placeholder": "想讓 codebus 文件化什麼？",
  "workspace.newGoalModal.cancel": "取消",
  "workspace.newGoalModal.run": "執行",
  "workspace.newGoalModal.blockedHint":
    "請等目前的 goal 結束或先取消，再啟動新的 goal。",

  // ---- Workspace · Status pill labels (Phase 3B three-state) ----
  "workspace.status.done": "完成",
  "workspace.status.interrupted": "已中斷",
  "workspace.status.failed": "失敗",
  "workspace.status.running": "執行中",

  // ---- Workspace · Run detail · Running ----
  "workspace.runDetail.backLink": "← 回上頁",
  "workspace.runDetail.runningBadge": "⏺ 進行中",
  "workspace.runDetail.cancelButton": "⏹ 取消",
  "workspace.runDetail.cancellingButton": "取消中…",

  // ---- Workspace · Run detail · Done / Cancelled / Interrupted ----
  "workspace.runDetail.doneBadge": "✓ 完成",
  "workspace.runDetail.coveredPagesLabel": "更新到的 wiki page",
  "workspace.runDetail.coveredPagesEmpty": "這次 run 沒更動到 wiki 頁面",
  "workspace.runDetail.lintLabel": "Lint",
  "workspace.runDetail.activitySummaryLabel": "活動摘要",
  "workspace.runDetail.toolReadLine": "{n} 次 Read",
  "workspace.runDetail.toolGlobLine": "{n} 次 Glob",
  "workspace.runDetail.toolGrepLine": "{n} 次 Grep",
  "workspace.runDetail.toolWriteLine": "{n} 次 Write",
  "workspace.runDetail.toolEditLine": "{n} 次 Edit",
  "workspace.runDetail.toolOtherLine": "{n} 次 {tool}",
  "workspace.runDetail.thinkingLabel": "思考",
  "workspace.runDetail.showThinking": "展開思考 ▼",
  "workspace.runDetail.hideThinking": "收起思考 ▲",
  "workspace.runDetail.showDetails": "展開完整 timeline ▼",
  "workspace.runDetail.hideDetails": "收起完整 timeline ▲",
  "workspace.runDetail.phaseGoal": "goal 階段",
  "workspace.runDetail.phaseFix": "fix 階段",
  "workspace.runDetail.phaseQuery": "query 階段",
  "workspace.runDetail.phaseChat": "chat 階段",
  "workspace.runDetail.phaseOther": "{verb} 階段",
  "workspace.runDetail.phaseEmptyHint": "（無 tool 使用）",
  "workspace.runDetail.coveredPagesPhaseEmpty": "（無 wiki page 變更）",
  "workspace.runDetail.cancelledBadge": "⏹ 已取消",
  "workspace.runDetail.cancelledWarning":
    "Wiki 仍有未 commit 的變更 — 沒自動 commit。需要時請到 terminal 檢查。",
  "workspace.runDetail.interruptedBadge": "⚠ 中斷",
  "workspace.runDetail.interruptedWarning":
    "App 被關閉，goal 沒完成。Wiki 可能停在中間狀態 — 需要時請到 terminal 檢查。",
  "workspace.runDetail.partialTimelineLabel": "部分時間軸",
  "workspace.runDetail.retryButton": "用相同 goal 再跑一次",
  "workspace.run.headerSummary": "{durationSec} 秒 · {totalTokens} tokens",
  "workspace.run.lintSummary": "{errors} 個錯誤 · {warnings} 個警告",

  // ---- Workspace · Wiki ----
  "workspace.wiki.empty":
    "目前還沒有 wiki page — 跑一個 goal 開始整理文件",
  "workspace.wiki.toggleTreeAria": "切換 Pages 樹狀結構",
  "workspace.wiki.pageNotFound": "找不到頁面",
  "workspace.wiki.openInObsidian": "在 Obsidian 開啟",

  // ---- Workspace · Quiz placeholder ----
  "workspace.quiz.placeholder":
    "Coming soon — quiz flow ships in v3-app-quiz",

  // ---- Chat (cmdk) ----
  "chat.onboarding.hintEn":
    "Ask anything about this vault. AI will suggest [Promote to goal] when it fits, or you can ask AI to promote a message to a goal.",
  "chat.onboarding.hintTw":
    "可以問任何關於此 vault 的事。AI 覺得適合時會主動建議 [Promote to goal]，你也可以主動跟 AI 講想做成 goal（例如「幫我把這段寫成 goal」）。",
  "chat.placeholder.en": "Ask anything…",
  "chat.placeholder.tw": "輸入訊息…",
  "chat.button.newChat": "+ 新對話",
  "chat.button.stop": "⏹ 停止",
  "chat.button.send": "送出",
  "chat.button.promote": "設為 goal",
  "chat.button.dismiss": "忽略",
  "chat.toast.startedNewChat": "已開始新對話。",
  "chat.toast.undo": "復原",
  "chat.undoToast.heading": "🆕 已開始新對話",
  "chat.undoToast.countdown": "（{n} 秒可復原）",
  "chat.error.anotherGoalRunning": "目前有其他 goal 在執行，等它結束。",
  "chat.error.promoteFailed": "Promote 失敗，請再試一次。",
  "chat.token.tooltip.cacheRead": "快取讀取",
  "chat.token.tooltip.cacheCreate": "快取建立",
  "chat.token.tooltip.input": "輸入",
  "chat.token.tooltip.output": "輸出",
  "chat.widget.aria.openChat": "開啟對話",
  "chat.widget.aria.openChatWithActiveGoalRunning":
    "開啟對話 — 有目標正在執行",
  "chat.widget.aria.closeChat": "關閉對話",

  // ---- Errors ----
  "errors.vaultAlreadyExists": "這個 vault 已經在清單裡了：{path}",
  "errors.vaultNotFound": "路徑已不存在：{path}",
  "errors.invalid": "{field}：{message}",
  "errors.io": "檔案系統錯誤：{message}",
  "errors.configParse": "Config 解析錯誤：{message}",
  "errors.internal": "{message}",
  "errors.generic": "發生未預期的錯誤",

  // ---- Settings · Endpoint sections (Cat A sweep) ----
  "settings.endpoint.claude.heading": "Claude Code 端點設定",
  "settings.endpoint.codex.heading": "OpenAI Codex 端點設定",
  "settings.endpoint.activeProfileAria": "目前使用的端點 profile",
  "settings.endpoint.activeProfileAriaCodex": "目前使用的 codex 端點 profile",
  "settings.endpoint.profile.system": "系統",
  "settings.endpoint.profile.azure": "Azure",
  "settings.endpoint.profile.systemTitle": "系統 Profile",
  "settings.endpoint.profile.azureTitle": "Azure Profile",
  "settings.endpoint.profile.inactiveLabel": "（未啟用）",
  "settings.endpoint.field.apiKey": "API 金鑰",
  "settings.endpoint.field.effort": "effort",
  "settings.endpoint.placeholder.claudeModel": "<model，例：opus-4-7>",
  "settings.endpoint.placeholder.codexModel": "<model，例：gpt-5.5>",
  "settings.endpoint.placeholder.deploymentName": "<deployment 名稱>",
  "settings.endpoint.placeholder.azureBaseUrlClaude":
    "https://<resource>.cognitiveservices.azure.com/anthropic",
  "settings.endpoint.placeholder.azureBaseUrlCodex":
    "https://<resource>.cognitiveservices.azure.com/openai",
  "settings.endpoint.placeholder.apiVersion": "2025-04-01-preview",
  "settings.endpoint.placeholder.codexEffort": "effort",
  "settings.endpoint.keyStatus.set": "已設定",
  "settings.endpoint.keyStatus.unset": "未設定",
  "settings.endpoint.keyStatus.unknown": "—",
  "settings.endpoint.keySetNew": "設定新值…",
  "settings.endpoint.keyDelete": "刪除",
  "settings.endpoint.validationSummaryHeading": "端點設定不完整：",
  "settings.endpoint.saveButtonIncompleteTitle":
    "端點設定不完整 — 請修正標紅的欄位",
  "settings.endpoint.validation.azureProfileRequired":
    "當 active=azure 時必須填 Azure profile",
  "settings.endpoint.validation.baseUrlRequired":
    "當 active=azure 時 base_url 為必填",
  "settings.endpoint.validation.apiVersionRequired":
    "當 active=azure 時 api_version 為必填",
  "settings.endpoint.validation.keyringServiceRequired":
    "當 active=azure 時 keyring_service 為必填",
  "settings.endpoint.validation.deploymentNameRequired":
    "當 active=azure 時 {verb} 的 deployment 名稱為必填",
  "settings.endpoint.validation.effortInvalid":
    "{verb} 的 effort 必須是 {allowed} 之一",
  "settings.endpoint.validation.systemModelRequired":
    "當 active=system 時 {verb} 的 model 為必填",

  // ---- Settings · jargon allow-list (Cat D) — same as en ----
  "settings.endpoint.field.baseUrl": "base_url",
  "settings.endpoint.field.apiVersion": "api_version",
  "settings.endpoint.field.keyringService": "keyring_service",
  "settings.endpoint.verb.goal": "goal",
  "settings.endpoint.verb.query": "query",
  "settings.endpoint.verb.fix": "fix",
  "settings.endpoint.verb.verify": "verify",
  "settings.endpoint.codex.effort.low": "low",
  "settings.endpoint.codex.effort.medium": "medium",
  "settings.endpoint.codex.effort.high": "high",
  "settings.endpoint.codex.effort.xhigh": "xhigh",

  // ---- Settings · SetKeyDialog (Cat A sweep) ----
  "settings.setKeyDialog.title": "設定 Azure API 金鑰",
  "settings.setKeyDialog.inputLabel":
    "貼上 API 金鑰 — 會存進作業系統 keyring，不會寫入 ~/.codebus/config.yaml。",
  "settings.setKeyDialog.errorEmpty": "API 金鑰不能空白",

  // ---- Settings · CLI install status (residual sweep) ----
  "settings.cliStatus.checking": "檢查中…",
  "settings.cliStatus.installed": "已安裝 · {version}",
  "settings.cliStatus.notInstalled": "未安裝",
  "settings.fields.pii.scannerNone": "none",

  // ---- Workspace · QuizAnswering (Cat B sweep) ----
  "workspace.quiz.answering.questionCounter": "第 {n} 題 / 共 {total} 題",
  "workspace.quiz.answering.parseEmpty":
    "Quiz 無法解析 — 沒有合格題目。",
  "workspace.quiz.answering.summaryHeading": "Quiz 完成",
  "workspace.quiz.answering.scoreLine":
    "得分：{correct} / {total}（{percent}%）",
  "workspace.quiz.answering.outcomePassed": "通過（門檻 {n}%）",
  "workspace.quiz.answering.outcomeFailed": "未通過（門檻 {n}%）",
  "workspace.quiz.answering.verdictCorrect": "答對",
  "workspace.quiz.answering.verdictIncorrect": "答錯",
  "workspace.quiz.answering.submitButton": "送出",
  "workspace.quiz.answering.nextButton": "下一題",
  "workspace.quiz.answering.finishButton": "完成",

  // ---- Workspace · QuizReview (Cat B sweep) ----
  "workspace.quiz.review.backToHistory": "← 回到歷史",
  "workspace.quiz.review.redoButton": "重做此份",
  "workspace.quiz.review.viewLogButton": "看過程",
  "workspace.quiz.review.viewLogClose": "關閉",
  "workspace.quiz.review.summaryLine":
    "{correct} / {total}（{percent}%）— {outcome}",
  "workspace.quiz.review.yourAnswerLine":
    "你的答案：{selected} · 正確答案：{correct}",
  "workspace.quiz.review.generationLogTitle": "生成記錄",

  // ---- Workspace · QuizTab (Cat B sweep) ----
  "workspace.quiz.tab.heading": "Quiz 歷史",
  "workspace.quiz.tab.newButton": "+ 新增 Quiz",
  "workspace.quiz.tab.emptyHint":
    "目前還沒 quiz — 用 + 新增 Quiz 開始",
  "workspace.quiz.tab.startButton": "開始",
  "workspace.quiz.tab.topicPlaceholder": "想被測什麼？",
  "workspace.quiz.tab.backToHistoryShort": "← 歷史",
  "workspace.quiz.tab.backToHistoryFull": "← 回到歷史",
  "workspace.quiz.tab.planningStatus": "規劃 quiz 範圍中…",
  "workspace.quiz.tab.generatingStatus": "生成題目中…",
  "workspace.quiz.tab.noMatchPrefix": "沒有符合的 wiki 頁面：{reason}",
  "workspace.quiz.tab.errorPrefix": "Quiz 失敗：{message}",
  "workspace.quiz.tab.backButton": "返回",
  "workspace.quiz.generationLogLoadError": "無法載入生成記錄：{error}",
  "workspace.quiz.headerTitle": "Quiz",
  "workspace.quiz.headerSubtitle": "驗證自己有沒有看懂 wiki。",

  // ---- Workspace · WikiPreview action (residual sweep) ----
  "workspace.wiki.quizMeOnThis": "考我這頁",

  // ---- Workspace · Run detail loading state (residual sweep) ----
  "workspace.runDetail.loading": "載入中…",

  // ---- a11y (Cat C sweep) — shared accessibility keys ----
  "a11y.dialogClose": "關閉",
  "chat.widget.aria.resizeChat": "調整聊天視窗大小",
  "chat.widget.aria.minimizeChat": "縮小聊天視窗",
  "chat.widget.title.dragToResize": "拖曳以調整大小",
  "chat.widget.title.minimizeShortcut": "縮小（Cmd+K）",

  // ---- Activity stream · internal sentinel markers ----
  "activity.marker.codebusQuizNoValidate":
    "codex 沙箱無法跑 quiz 結構驗證，跳過此步",

  // ---- Workspace · ActivityStream banner labels ----
  "workspace.activity.banner.start":
    "🚌 來囉來囉~ CodeBus 駛入 {path}...",
  "workspace.activity.banner.goal": "🎯 任務目標：{goalText}",
  "workspace.activity.banner.syncStart": "🔄 同步 source → raw/code...",
  "workspace.activity.banner.syncDone":
    "✓ 同步完成 ({files} 檔, {mib} MiB, {elapsedMs} ms)",
  "workspace.activity.banner.piiSummary":
    "🛡 PII：{scanner}, scanned {scanned}, hits {hits}, action {action}",
  "workspace.activity.banner.lintStart": "🔍 lint 中...",
  "workspace.activity.banner.lintDone":
    "✓ lint 完成 ({errors} errors, {warns} warns, {elapsedMs} ms)",
  "workspace.activity.banner.commitDone": "🚏 commit {sha7}",
  "workspace.activity.banner.done": "🎉 完成",
  "workspace.activity.banner.hint": "💡 提示",

  // ---- Quiz badge verdict ----
  "quiz.badge.pass": "通過",
  "quiz.badge.fail": "未通過",

  // ---- Settings provider CLI field label ----
  "settings.providerCli.fieldLabel": "{name} CLI",
  "settings.providerCli.installHint": "請先安裝 {name}，再重新開啟設定。",

  // ---- Settings · Language dropdown ----
  // "中文" / "English" 在兩個 locale 文字相同（identifier 性質，Cat D）。
  "settings.language.label": "語系",
  "settings.language.auto": "自動（依系統）",
  "settings.language.zh": "中文",
  "settings.language.en": "English",

  // ---- Chat token usage header indicator ----
  "chat.tokens.indicator": "{value} ↑",

  // ---- Workspace · ActivityStream 2-phase cluster ----
  "workspace.activity.cluster.reading.heading": "讀檔案",
  "workspace.activity.cluster.writing.heading": "寫 wiki",
  "workspace.activity.cluster.expand": "展開 cluster",
  "workspace.activity.cluster.collapse": "收合 cluster",
  "workspace.activity.cluster.summary.reading":
    "讀檔案 {reads} 次 · shell {shell} 次 · {elapsedSeconds} 秒",
  "workspace.activity.cluster.summary.writing":
    "新增 {new} · 更新 {updated} · {elapsedSeconds} 秒",

  // ---- Workspace · Quiz wizard ----
  // 5-bucket identifier 為 Cat D（identifier 性質），bucketIdentifier.*
  // 兩 locale 都保持英文字面；bucketLabel.* 是顯示用 prose，可在地化。
  "workspace.quiz.wizard.step1.title": "選一個主題",
  "workspace.quiz.wizard.step1.subtitle":
    "寫下你想被 quiz 的範圍 — codebus 會挑相關 wiki 頁面出題。",
  "workspace.quiz.wizard.step1.placeholder":
    "例如：auth 怎麼運作 / IM 適配器系統 / 對話傳遞流程",
  "workspace.quiz.wizard.step1.examplePillHint":
    "點擊範例直接填入。按 Enter 送出。",
  "workspace.quiz.wizard.step2.title": "確認範圍",
  "workspace.quiz.wizard.step2.bucketLabel.concepts": "概念",
  "workspace.quiz.wizard.step2.bucketLabel.entities": "實體",
  "workspace.quiz.wizard.step2.bucketLabel.modules": "模組",
  "workspace.quiz.wizard.step2.bucketLabel.processes": "流程",
  "workspace.quiz.wizard.step2.bucketLabel.synthesis": "綜整",
  "workspace.quiz.wizard.step2.bucketIdentifier.concepts": "concepts",
  "workspace.quiz.wizard.step2.bucketIdentifier.entities": "entities",
  "workspace.quiz.wizard.step2.bucketIdentifier.modules": "modules",
  "workspace.quiz.wizard.step2.bucketIdentifier.processes": "processes",
  "workspace.quiz.wizard.step2.bucketIdentifier.synthesis": "synthesis",
  "workspace.quiz.wizard.step3.title": "出題中",
  "workspace.quiz.wizard.step3.generatingHint":
    "CodeBus 正在閱讀 wiki 頁面、撰寫題目⋯",
  "workspace.quiz.wizard.step4.pendingTitle": "Quiz 已準備好",
  "workspace.quiz.wizard.step4.reviewingTitle": "Quiz：{topic}",
  "workspace.quiz.wizard.step4.completionTitle.pass": "通過了（{percent}%）",
  "workspace.quiz.wizard.step4.completionTitle.fail": "沒通過（{percent}%）",
  "workspace.quiz.wizard.action.cancel": "取消",
  "workspace.quiz.wizard.action.back": "返回",
  "workspace.quiz.wizard.action.next": "下一步 →",
  "workspace.quiz.wizard.action.start": "開始",
  "workspace.quiz.wizard.action.submit": "送出",
  "workspace.quiz.wizard.action.retry": "重試",
  "workspace.quiz.wizard.action.redo": "↻ 重做此份",
  "workspace.quiz.wizard.action.viewWrong": "看錯題",
  "workspace.quiz.wizard.action.viewProcess": "看過程",
  "workspace.quiz.wizard.header.stepLabel": "Step {n} / {total} · {name}",
}

export const messages = { en, zh } as const
export type MessageKey = keyof typeof en
