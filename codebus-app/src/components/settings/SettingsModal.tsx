import { useEffect, useState } from "react"
import { RotateCcw } from "lucide-react"

import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Slider } from "@/components/ui/slider"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { useSettingsStore } from "@/store/settings"
import { useT, type TFunction } from "@/i18n/useT"
import {
  type CliStatus,
  checkCliInstalled,
  validateClaudeCodeBlock,
  validateCodexBlock,
} from "@/lib/ipc"
import { PROVIDERS, type ProviderId } from "@/lib/providers"
import { EndpointSection } from "./EndpointSection"
import { CodexEndpointSection } from "./CodexEndpointSection"

/**
 * Default value table for the reset-to-default affordance. Pinned to match
 * codebus-core's per-section `Default::default()` impls + CLI starter
 * config. Drift risk is low (these values are spec-locked) but if a future
 * change moves any of them, update this table at the same time.
 *
 * As of `stage-b-app-endpoint-settings`, model defaults moved into
 * `<EndpointSection>` (which reads them from `SYSTEM_PROFILE_DEFAULTS`
 * exported by `lib/ipc.ts`). PII / quiz defaults remain here.
 */
const FIELD_DEFAULTS = {
  piiScanner: "regex_basic",
  passThreshold: 80,
  defaultLength: 5,
} as const

function ResetButton({
  isDefault,
  onReset,
  testId,
  t,
}: {
  isDefault: boolean
  onReset: () => void
  testId?: string
  t: TFunction
}) {
  const label = isDefault
    ? t("settings.reset.alreadyDefault")
    : t("settings.reset.label")
  return (
    <button
      type="button"
      onClick={onReset}
      disabled={isDefault}
      aria-label={label}
      title={label}
      data-testid={testId}
      className="text-fg-tertiary hover:text-fg disabled:hover:text-fg-tertiary disabled:opacity-30 rounded-sm p-1 focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-ring"
    >
      <RotateCcw className="h-3 w-3" />
    </button>
  )
}

interface SettingsModalProps {
  open: boolean
  onClose: () => void
  /**
   * Runtime PII pattern count (from `codebus_core::pii::scanners` registry).
   * Surfaced as a prop so the markup never hard-codes the count.
   */
  piiPatternCount: number
}

export function SettingsModal({
  open,
  onClose,
  piiPatternCount,
}: SettingsModalProps) {
  const t = useT()
  const config = useSettingsStore((s) => s.config)
  const dirty = useSettingsStore((s) => s.dirty)
  const saving = useSettingsStore((s) => s.saving)
  const error = useSettingsStore((s) => s.error)
  const load = useSettingsStore((s) => s.load)
  const update = useSettingsStore((s) => s.update)
  const save = useSettingsStore((s) => s.save)
  const reset = useSettingsStore((s) => s.reset)
  const getClaudeCodeBlock = useSettingsStore((s) => s.getClaudeCodeBlock)
  const updateClaudeCode = useSettingsStore((s) => s.updateClaudeCode)
  const getCodexBlock = useSettingsStore((s) => s.getCodexBlock)
  const updateProviderBlock = useSettingsStore((s) => s.updateProviderBlock)
  const setActiveProvider = useSettingsStore((s) => s.setActiveProvider)

  // The selected provider drives which endpoint editor + CLI probe render.
  const providerId =
    ((config as { agent?: { active_provider?: string } } | null)?.agent
      ?.active_provider as ProviderId) ?? "claude"
  const provider = PROVIDERS[providerId] ?? PROVIDERS.claude

  const [saved, setSaved] = useState(false)
  const [cliStatus, setCliStatus] = useState<CliStatus | "checking" | null>(null)

  // Load config on open / reset on close. MUST NOT depend on the selected
  // provider — re-running `load()` on a provider switch would reload the
  // on-disk config and discard the in-memory switch (reverting the selector).
  useEffect(() => {
    if (open) {
      load()
    } else {
      reset()
      setSaved(false)
      setCliStatus(null)
    }
  }, [open, load, reset])

  // Probe the selected provider's CLI whenever the provider changes (while
  // open). Separate from `load()` so switching providers only re-probes.
  useEffect(() => {
    if (!open) return
    setCliStatus("checking")
    checkCliInstalled(provider.cliBinaryId as never)
      .then(setCliStatus)
      .catch(() => setCliStatus({ kind: "not_installed" }))
  }, [open, provider.cliBinaryId])

  const safeConfig = (config ?? {}) as {
    app?: { quiz?: { pass_threshold?: number; default_length?: number } }
    quiz?: { default_length?: number; content_verify?: boolean }
    goal?: { content_verify?: boolean }
    pii?: { scanner?: string; on_hit?: string; patterns_extra?: string[] }
    lint?: { fix?: { enabled?: boolean } }
    log?: { sink?: string; dir?: string }
    hooks?: { read_image_block?: boolean }
  }
  const passThreshold = safeConfig.app?.quiz?.pass_threshold ?? 80
  // default_length moved to the shared top-level `quiz.*` namespace
  // (v3-app-quiz). Prefer the shared key; fall back to a legacy
  // app.quiz.default_length still present in an un-migrated config; then 5.
  const defaultLength =
    safeConfig.quiz?.default_length ?? safeConfig.app?.quiz?.default_length ?? 5
  const claudeCode = getClaudeCodeBlock()
  const codexBlock = getCodexBlock()
  // Validate only the active provider's endpoint block; that gates Save.
  const endpointErrors =
    providerId === "codex"
      ? validateCodexBlock(codexBlock)
      : validateClaudeCodeBlock(claudeCode)
  const claudeCodeValid = endpointErrors.length === 0

  const piiScanner = safeConfig.pii?.scanner ?? "regex_basic"
  const logDir = safeConfig.log?.dir ?? ""

  // --- settings-config-frontend: newly surfaced config knobs ---
  const piiOnHit = safeConfig.pii?.on_hit ?? "warn"
  const lintFixEnabled = safeConfig.lint?.fix?.enabled ?? true
  const quizContentVerify = safeConfig.quiz?.content_verify ?? false
  const goalContentVerify = safeConfig.goal?.content_verify ?? false
  const loggingDisabled = safeConfig.log?.sink === "none"
  // pretouseluse-image-block-toggle: default true (block) when config
  // omits the hooks section, matching `HooksConfig::default()` in Rust.
  const readImageBlock = safeConfig.hooks?.read_image_block ?? true
  const patternsExtra: string[] = safeConfig.pii?.patterns_extra ?? []
  // A pattern is invalid when it cannot compile as a RegExp. Empty entries
  // (freshly added rows) are treated as not-yet-invalid so the user can
  // type into them without Save being blocked prematurely.
  const patternInvalid = (p: string): boolean => {
    if (p.length === 0) return false
    try {
      // eslint-disable-next-line no-new
      new RegExp(p)
      return false
    } catch {
      return true
    }
  }
  const piiPatternsInvalid = patternsExtra.some(patternInvalid)

  function setPatternsExtra(next: string[]) {
    update({
      pii: { patterns_extra: next } as unknown as Record<string, unknown>,
    } as never)
  }

  async function handleSave() {
    try {
      await save()
      setSaved(true)
      // Close on next tick so the success toast can fade in.
      setTimeout(() => {
        onClose()
        setSaved(false)
      }, 250)
    } catch {
      // The store sets `error`; the modal stays open and shows inline error.
    }
  }

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent data-testid="settings-modal">
        <DialogHeader>
          <DialogTitle>{t("settings.title")}</DialogTitle>
        </DialogHeader>
        <div
          data-testid="settings-form"
          className="grid max-h-[60vh] grid-cols-[168px_1fr] gap-x-4 gap-y-3 overflow-auto p-4 text-xs"
        >
          {/* 1. AI Provider — selector. Switches active_provider, which
             re-routes the endpoint editor + CLI status row below. */}
          <Field label={t("settings.fields.aiProvider.label")}>
            <Select
              value={providerId}
              onValueChange={(v) => setActiveProvider(v)}
            >
              <SelectTrigger className="w-[200px]" data-testid="ai-provider-select">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {Object.values(PROVIDERS).map((p) => (
                  <SelectItem key={p.id} value={p.id} data-testid={`ai-provider-${p.id}`}>
                    {p.displayName}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>

          {/* 2. CLI status — probes whether `claude --version` works.
             Replaces the v1 OAuth pseudo-status with a real installation
             check; future Codex / Gemini providers add their own rows. */}
          <Field label={`${provider.displayName} CLI`}>
            <div className="flex items-center gap-2">
              <CliStatusBadge status={cliStatus} />
              {cliStatus &&
                cliStatus !== "checking" &&
                cliStatus.kind === "not_installed" && (
                  <span
                    className="text-xs text-fg-secondary"
                    data-testid="cli-install-hint"
                  >
                    Install {provider.displayName} first; then reopen Settings.
                  </span>
                )}
            </div>
          </Field>

          {/* 3. Endpoint settings (system / azure profile) — supersedes
             the legacy "Default model per verb" dropdowns; lives in its
             own component because the form has two non-trivial profile
             sub-sections + keyring management. */}
          {providerId === "codex" ? (
            <CodexEndpointSection
              block={codexBlock}
              onChange={(next) => updateProviderBlock("codex", next)}
              errors={endpointErrors}
            />
          ) : (
            <EndpointSection
              claudeCode={claudeCode}
              onChange={updateClaudeCode}
              errors={endpointErrors}
            />
          )}


          {/* 4. PII scanner — dynamic count */}
          <Field label={t("settings.fields.pii.label")}>
            <div className="flex items-center gap-2">
              <Select
                value={piiScanner}
                onValueChange={(value) =>
                  update({
                    pii: { scanner: value } as Record<string, unknown>,
                  } as never)
                }
              >
                <SelectTrigger
                  className="w-[260px]"
                  data-testid="pii-scanner-trigger"
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="regex_basic">
                    {t("settings.fields.pii.display", {
                      count: piiPatternCount,
                    })}
                  </SelectItem>
                  <SelectItem value="none">none</SelectItem>
                </SelectContent>
              </Select>
              <ResetButton
                isDefault={piiScanner === FIELD_DEFAULTS.piiScanner}
                onReset={() =>
                  update({
                    pii: {
                      scanner: FIELD_DEFAULTS.piiScanner,
                    } as Record<string, unknown>,
                  } as never)
                }
                testId="reset-pii-scanner"
                t={t}
              />
            </div>
            <div
              data-testid="pii-pattern-count-display"
              className="text-[11px] text-fg-tertiary"
            >
              {t("settings.fields.pii.display", { count: piiPatternCount })}
            </div>
            {/* Stable hook for the snapshot test asserting runtime-driven count. */}
            <span hidden data-testid="pii-pattern-count">
              {piiPatternCount}
            </span>
          </Field>

          {/* 4b. PII on-hit policy */}
          <Field label={t("settings.fields.piiOnHit.label")}>
            <div className="flex items-center gap-2">
              <select
                data-testid="pii-on-hit-select"
                value={piiOnHit}
                onChange={(e) =>
                  update({
                    pii: { on_hit: e.target.value } as Record<string, unknown>,
                  } as never)
                }
                className="rounded-md border border-border bg-bg-raised px-2 py-1 text-xs"
              >
                <option value="warn">
                  {t("settings.fields.piiOnHit.warn")}
                </option>
                <option value="skip">
                  {t("settings.fields.piiOnHit.skip")}
                </option>
                <option value="mask">
                  {t("settings.fields.piiOnHit.mask")}
                </option>
              </select>
            </div>
            <div
              data-testid="pii-on-hit-critical-note"
              className="text-[11px] text-fg-tertiary"
            >
              {t("settings.fields.piiOnHit.criticalNote")}
            </div>
          </Field>

          {/* 4c. PII extra patterns */}
          <Field label={t("settings.fields.piiPatterns.label")}>
            <div className="flex flex-col gap-1">
              {patternsExtra.map((p, i) => {
                const invalid = patternInvalid(p)
                return (
                  <div key={i} className="flex flex-col gap-0.5">
                    <div className="flex items-center gap-2">
                      <input
                        data-testid={`pii-patterns-input-${i}`}
                        value={p}
                        placeholder={t(
                          "settings.fields.piiPatterns.placeholder",
                        )}
                        onChange={(e) => {
                          const next = [...patternsExtra]
                          next[i] = e.target.value
                          setPatternsExtra(next)
                        }}
                        className={`w-[260px] rounded-md border bg-bg-raised px-2 py-1 font-mono text-[11px] ${
                          invalid
                            ? "border-error focus-visible:ring-error"
                            : "border-border"
                        }`}
                        aria-invalid={invalid || undefined}
                      />
                      <button
                        type="button"
                        data-testid={`pii-patterns-remove-${i}`}
                        onClick={() =>
                          setPatternsExtra(
                            patternsExtra.filter((_, j) => j !== i),
                          )
                        }
                        className="text-xs text-fg-secondary underline decoration-dashed hover:text-fg"
                      >
                        {t("settings.fields.piiPatterns.remove")}
                      </button>
                    </div>
                    {invalid && (
                      <span
                        data-testid={`pii-patterns-error-${i}`}
                        className="text-[11px] text-error"
                      >
                        {t("settings.fields.piiPatterns.invalid")}
                      </span>
                    )}
                  </div>
                )
              })}
              <button
                type="button"
                data-testid="pii-patterns-add"
                onClick={() => setPatternsExtra([...patternsExtra, ""])}
                className="self-start text-xs text-fg-secondary underline decoration-dashed hover:text-fg"
              >
                {t("settings.fields.piiPatterns.add")}
              </button>
            </div>
          </Field>

          {/* 4d. Lint fix enabled */}
          <Field
            label={t("settings.fields.lintFix.label")}
            subLabel={t("settings.fields.lintFix.sublabel")}
          >
            <label className="flex items-center gap-2 text-xs">
              <input
                type="checkbox"
                data-testid="lint-fix-toggle"
                checked={lintFixEnabled}
                onChange={() =>
                  update({
                    lint: {
                      fix: { enabled: !lintFixEnabled },
                    } as Record<string, unknown>,
                  } as never)
                }
              />
            </label>
          </Field>

          {/* 4e. Quiz content verify */}
          <Field label={t("settings.fields.quizContentVerify.label")}>
            <label className="flex items-center gap-2 text-xs">
              <input
                type="checkbox"
                data-testid="quiz-content-verify-toggle"
                checked={quizContentVerify}
                onChange={() =>
                  update({
                    quiz: {
                      content_verify: !quizContentVerify,
                    } as Record<string, unknown>,
                  } as never)
                }
              />
            </label>
            <div
              data-testid="quiz-content-verify-cost"
              className="text-[11px] text-fg-tertiary"
            >
              {t("settings.fields.quizContentVerify.cost")}
            </div>
          </Field>

          {/* 4f. Goal content verify */}
          <Field label={t("settings.fields.goalContentVerify.label")}>
            <label className="flex items-center gap-2 text-xs">
              <input
                type="checkbox"
                data-testid="goal-content-verify-toggle"
                checked={goalContentVerify}
                onChange={() =>
                  update({
                    goal: {
                      content_verify: !goalContentVerify,
                    } as Record<string, unknown>,
                  } as never)
                }
              />
            </label>
            <div
              data-testid="goal-content-verify-cost"
              className="text-[11px] text-fg-tertiary"
            >
              {t("settings.fields.goalContentVerify.cost")}
            </div>
          </Field>

          {/* 4g. Read hook image block (pretouseluse-image-block-toggle) */}
          <Field label={t("settings.fields.readImageBlock.label")}>
            <label className="flex items-center gap-2 text-xs">
              <input
                type="checkbox"
                data-testid="read-image-block-toggle"
                checked={readImageBlock}
                onChange={() =>
                  update({
                    hooks: {
                      read_image_block: !readImageBlock,
                    } as Record<string, unknown>,
                  } as never)
                }
              />
            </label>
            <div
              data-testid="read-image-block-warning"
              className="text-[11px] text-fg-tertiary"
            >
              {t("settings.fields.readImageBlock.warning")}
            </div>
          </Field>

          {/* 5. Log sink */}
          <Field label={t("settings.fields.logSink.label")}>
            <div className="flex items-center gap-2">
              <span
                data-testid="log-sink-path"
                className="rounded-md border border-border bg-bg-raised px-2 py-1 font-mono text-[11px] text-fg-secondary"
              >
                {logDir || t("settings.fields.logSink.perVaultDefault")}
              </span>
              <button
                data-testid="log-sink-change"
                onClick={async () => {
                  try {
                    const mod = await import("@tauri-apps/plugin-dialog")
                    const picked = await mod.open({
                      directory: true,
                      multiple: false,
                    })
                    if (typeof picked === "string") {
                      // jsonl is the only "active logging" sink today;
                      // setting dir implies turning the sink on.
                      update({
                        log: { sink: "jsonl", dir: picked } as Record<
                          string,
                          unknown
                        >,
                      } as never)
                    }
                  } catch (err) {
                    console.error("log sink picker failed", err)
                  }
                }}
                className="text-xs text-fg-secondary underline decoration-dashed hover:text-fg"
              >
                {t("settings.fields.logSink.change")}
              </button>
              <ResetButton
                isDefault={!logDir}
                onReset={() =>
                  update({
                    log: { sink: "jsonl", dir: null } as unknown as Record<
                      string,
                      unknown
                    >,
                  } as never)
                }
                testId="log-sink-reset"
                t={t}
              />
              <label className="flex items-center gap-1 text-xs text-fg-secondary">
                <input
                  type="checkbox"
                  data-testid="log-disable-toggle"
                  checked={loggingDisabled}
                  onChange={() =>
                    update({
                      log: {
                        sink: loggingDisabled ? "jsonl" : "none",
                      } as Record<string, unknown>,
                    } as never)
                  }
                />
                <span>{t("settings.fields.logSink.disable")}</span>
              </label>
            </div>
          </Field>

          {/* 6. Quiz pass threshold */}
          <Field
            label={t("settings.fields.quizThreshold.label")}
            subLabel={t("settings.fields.quizThreshold.sublabel")}
            testId="quiz-threshold-field"
          >
            <div className="flex items-center gap-3">
              <Slider
                value={[passThreshold]}
                min={50}
                max={100}
                step={1}
                onValueChange={([v]) =>
                  update({
                    app: { quiz: { pass_threshold: v } } as Record<string, unknown>,
                  } as never)
                }
                className="flex-1"
              />
              <span
                data-testid="quiz-threshold-value"
                className="font-mono text-xs"
              >
                {t("settings.fields.quizThreshold.value", { n: passThreshold })}
              </span>
              <ResetButton
                isDefault={passThreshold === FIELD_DEFAULTS.passThreshold}
                onReset={() =>
                  update({
                    app: {
                      quiz: { pass_threshold: FIELD_DEFAULTS.passThreshold },
                    } as Record<string, unknown>,
                  } as never)
                }
                testId="reset-quiz-threshold"
                t={t}
              />
            </div>
          </Field>

          {/* 7. Default quiz length */}
          <Field label={t("settings.fields.quizLength.label")}>
            <div className="flex items-center gap-3">
              <Slider
                value={[defaultLength]}
                min={3}
                max={10}
                step={1}
                onValueChange={([v]) =>
                  update({
                    quiz: { default_length: v } as Record<string, unknown>,
                  } as never)
                }
                className="flex-1"
              />
              <span
                data-testid="quiz-length-value"
                className="font-mono text-xs"
              >
                {t("settings.fields.quizLength.value", { n: defaultLength })}
              </span>
              <ResetButton
                isDefault={defaultLength === FIELD_DEFAULTS.defaultLength}
                onReset={() =>
                  update({
                    quiz: {
                      default_length: FIELD_DEFAULTS.defaultLength,
                    } as Record<string, unknown>,
                  } as never)
                }
                testId="reset-quiz-length"
                t={t}
              />
            </div>
          </Field>
        </div>
        {error && (
          <div
            data-testid="settings-error"
            className="mx-4 mb-2 rounded-sm border border-error/40 bg-error/10 px-2 py-1 text-[11px] text-error"
          >
            {t(error.key, error.vars)}
          </div>
        )}
        {saved && (
          <div
            data-testid="settings-toast"
            className="mx-4 mb-2 rounded-sm border border-success/40 bg-success/10 px-2 py-1 text-[11px] text-success"
          >
            {t("settings.toast.saved")}
          </div>
        )}
        <DialogFooter>
          <span className="mr-auto font-mono text-[11px] text-fg-tertiary">
            {t("settings.footer.note")}
          </span>
          <Button variant="ghost" onClick={onClose} data-testid="settings-cancel">
            {t("common.cancel")}
          </Button>
          <Button
            variant="primary"
            onClick={handleSave}
            disabled={!dirty || saving || !claudeCodeValid || piiPatternsInvalid}
            data-testid="settings-save"
            title={
              !claudeCodeValid
                ? "Endpoint configuration is incomplete — fix highlighted fields"
                : undefined
            }
          >
            {saving ? t("common.saving") : t("common.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function CliStatusBadge({
  status,
}: {
  status: CliStatus | "checking" | null
}) {
  if (status === null) {
    return null
  }
  if (status === "checking") {
    return (
      <span
        data-testid="cli-status"
        data-state="checking"
        className="rounded-full border border-border bg-bg px-2 py-px font-mono text-[10px] text-fg-tertiary"
      >
        Checking…
      </span>
    )
  }
  if (status.kind === "installed") {
    return (
      <span
        data-testid="cli-status"
        data-state="installed"
        className="rounded-full border border-success/40 bg-success/10 px-2 py-px font-mono text-[10px] text-success"
      >
        Installed · {status.version}
      </span>
    )
  }
  return (
    <span
      data-testid="cli-status"
      data-state="not_installed"
      className="rounded-full border border-error/40 bg-error/10 px-2 py-px font-mono text-[10px] text-error"
    >
      Not installed
    </span>
  )
}

function Field({
  label,
  subLabel,
  testId,
  children,
}: {
  label: string
  subLabel?: string
  testId?: string
  children: React.ReactNode
}) {
  return (
    <>
      <div className="text-fg-secondary text-[11px]">
        <div>{label}</div>
        {subLabel && (
          <div
            data-testid={testId ? `${testId}-sublabel` : undefined}
            className="font-mono text-[10px] text-fg-tertiary"
          >
            {subLabel}
          </div>
        )}
      </div>
      <div data-testid={testId}>{children}</div>
    </>
  )
}

