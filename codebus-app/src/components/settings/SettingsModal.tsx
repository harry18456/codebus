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

/**
 * Default value table for the reset-to-default affordance. Pinned to match
 * codebus-core's per-section `Default::default()` impls + CLI starter
 * config. Drift risk is low (these values are spec-locked) but if a future
 * change moves any of them, update this table at the same time.
 */
const FIELD_DEFAULTS = {
  goalModel: "opus",
  queryModel: "haiku",
  fixModel: "sonnet",
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
  /** OAuth status surfaced as text. v1 has no live auth flow, so caller
   * provides a static label. */
  oauthStatus?: "connected" | "disconnected"
}

const MODEL_OPTIONS = ["opus", "sonnet", "haiku"] as const

export function SettingsModal({
  open,
  onClose,
  piiPatternCount,
  oauthStatus = "connected",
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

  const [saved, setSaved] = useState(false)

  useEffect(() => {
    if (open) {
      load()
    } else {
      reset()
      setSaved(false)
    }
  }, [open, load, reset])

  const safeConfig = (config ?? {}) as {
    app?: { quiz?: { pass_threshold?: number; default_length?: number } }
    pii?: { scanner?: string }
    log?: { sink?: string; dir?: string }
    claude_code?: Record<string, { model?: string } | undefined>
  }
  const passThreshold = safeConfig.app?.quiz?.pass_threshold ?? 80
  const defaultLength = safeConfig.app?.quiz?.default_length ?? 5

  const goalModel = readModel(safeConfig, "goal")
  const queryModel = readModel(safeConfig, "query")
  const fixModel = readModel(safeConfig, "fix")

  const piiScanner = safeConfig.pii?.scanner ?? "regex_basic"
  const logDir = safeConfig.log?.dir ?? ""

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
          {/* 1. AI Provider — read-only */}
          <Field label={t("settings.fields.aiProvider.label")}>
            <div className="flex items-center gap-2">
              <span className="text-fg">
                {t("settings.fields.aiProvider.value")}
              </span>
              <span className="font-mono text-[11px] text-fg-tertiary">
                {t("settings.fields.aiProvider.note")}
              </span>
            </div>
          </Field>

          {/* 2. Authentication */}
          <Field label={t("settings.fields.auth.label")}>
            <div className="flex items-center gap-2">
              <span
                data-testid="oauth-status"
                className="rounded-full border border-success/40 bg-success/10 px-2 py-px font-mono text-[10px] text-success"
              >
                {oauthStatus === "connected"
                  ? t("settings.fields.auth.connected")
                  : t("settings.fields.auth.disconnected")}
              </span>
              <button className="text-xs text-fg-secondary underline decoration-dashed hover:text-fg">
                {t("settings.fields.auth.reauthenticate")}
              </button>
            </div>
          </Field>

          {/* 3. Default model per verb */}
          <Field
            label={t("settings.fields.defaultModel.label")}
            subLabel={t("settings.fields.defaultModel.sublabel")}
            testId="default-model-field"
          >
            <div className="flex flex-col gap-2">
              {(["goal", "query", "fix"] as const).map((verb) => {
                const current =
                  verb === "goal"
                    ? goalModel
                    : verb === "query"
                      ? queryModel
                      : fixModel
                const defaultValue =
                  verb === "goal"
                    ? FIELD_DEFAULTS.goalModel
                    : verb === "query"
                      ? FIELD_DEFAULTS.queryModel
                      : FIELD_DEFAULTS.fixModel
                return (
                  <div key={verb} className="flex items-center gap-2">
                    <span className="font-mono w-[56px] text-fg-tertiary text-[11px]">
                      {verb}
                    </span>
                    <Select
                      value={current}
                      onValueChange={(value) =>
                        update({
                          claude_code: {
                            [verb]: { model: value },
                          } as Record<string, unknown>,
                        } as never)
                      }
                    >
                      <SelectTrigger className="w-[140px]">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {MODEL_OPTIONS.map((m) => (
                          <SelectItem key={m} value={m}>
                            {m}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                    <ResetButton
                      isDefault={current === defaultValue}
                      onReset={() =>
                        update({
                          claude_code: {
                            [verb]: { model: defaultValue },
                          } as Record<string, unknown>,
                        } as never)
                      }
                      testId={`reset-default-model-${verb}`}
                      t={t}
                    />
                  </div>
                )
              })}
            </div>
          </Field>

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
                    app: { quiz: { default_length: v } } as Record<string, unknown>,
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
                    app: {
                      quiz: { default_length: FIELD_DEFAULTS.defaultLength },
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
            disabled={!dirty || saving}
            data-testid="settings-save"
          >
            {saving ? t("common.saving") : t("common.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
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

function readModel(
  config: Record<string, unknown> | { claude_code?: unknown },
  verb: "goal" | "query" | "fix",
): string {
  const cc = (config as { claude_code?: Record<string, unknown> }).claude_code
  if (cc && typeof cc === "object") {
    const verbCfg = cc[verb] as { model?: string } | undefined
    if (verbCfg?.model) return verbCfg.model
  }
  return verb === "query" ? "haiku" : "sonnet"
}
