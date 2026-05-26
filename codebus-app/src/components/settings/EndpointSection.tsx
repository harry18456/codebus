import { useEffect, useState } from "react"

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  type ClaudeCodeBlock,
  type ClaudeCodeValidationError,
  type KeyStatus,
  DEFAULT_CLAUDE_AZURE_KEYRING_SERVICE,
  SYSTEM_EFFORTS,
  SYSTEM_MODELS,
  deleteEndpointKey,
  getEndpointKey,
} from "@/lib/ipc"

import { useT } from "@/i18n/useT"

import { SetKeyDialog } from "./SetKeyDialog"

/**
 * Settings UI Endpoint Section — renders the active-profile radio AND both
 * system + azure profile sub-sections simultaneously per spec
 * `app-shell / Settings UI Endpoint Section`. The non-active sub-section
 * is visually de-emphasised (opacity 0.7) but stays editable so the user
 * can prepare cold-storage configuration before switching.
 */
export interface EndpointSectionProps {
  claudeCode: ClaudeCodeBlock
  onChange: (next: ClaudeCodeBlock) => void
  /**
   * Validation errors computed by the parent (typically via
   * `validateClaudeCodeBlock` in `lib/ipc.ts`). Empty array means the
   * block is currently saveable. EndpointSection uses this to render
   * `aria-invalid` on the offending fields and surface an inline
   * summary above the Save button area.
   */
  errors?: ClaudeCodeValidationError[]
}

// verify-stage-independent-model: `verify` is the fourth editable verb
// row (spec `app-shell` `Settings UI Endpoint Section`). It SHALL render
// after `fix` to convey the "verification follows main action" sequence.
const VERBS = ["goal", "query", "fix", "verify"] as const
type Verb = (typeof VERBS)[number]

export function EndpointSection({
  claudeCode,
  onChange,
  errors = [],
}: EndpointSectionProps) {
  const t = useT()
  const hasError = (field: string) => errors.some((e) => e.field === field)
  // The keyring entry to read/write is the claude azure profile's
  // `keyring_service` (defaulting to the claude-specific default), so claude
  // and codex keys never collide.
  const keyringService =
    claudeCode.azure?.keyring_service?.trim() || DEFAULT_CLAUDE_AZURE_KEYRING_SERVICE
  const [keyStatus, setKeyStatus] = useState<KeyStatus | null>(null)
  const [keyError, setKeyError] = useState<string | null>(null)
  const [setKeyOpen, setSetKeyOpen] = useState(false)
  // Accordion expand state per profile. Auto-folds on active toggle
  // (see effect below); user can manually expand the non-active profile
  // to edit cold storage without flipping active.
  const [systemExpanded, setSystemExpanded] = useState(
    claudeCode.active === "system",
  )
  const [azureExpanded, setAzureExpanded] = useState(
    claudeCode.active === "azure",
  )

  useEffect(() => {
    void refreshKeyStatus()
  }, [])

  // Auto-fold on active change: expand the newly active sub-section,
  // collapse the previously active one. User manual expand of inactive
  // (later) is not undone here — only the active toggle drives the
  // auto-fold.
  useEffect(() => {
    if (claudeCode.active === "system") {
      setSystemExpanded(true)
      setAzureExpanded(false)
    } else {
      setSystemExpanded(false)
      setAzureExpanded(true)
    }
  }, [claudeCode.active])

  async function refreshKeyStatus() {
    try {
      const status = await getEndpointKey(keyringService)
      setKeyStatus(status)
      setKeyError(null)
    } catch (err) {
      setKeyStatus(null)
      setKeyError(formatError(err))
    }
  }

  function setActive(next: "system" | "azure") {
    onChange({ ...claudeCode, active: next })
  }

  function setSystemModel(verb: Verb, model: string) {
    onChange({
      ...claudeCode,
      system: {
        ...claudeCode.system,
        [verb]: { ...claudeCode.system[verb], model },
      },
    })
  }

  function setSystemEffort(verb: Verb, effort: string) {
    onChange({
      ...claudeCode,
      system: {
        ...claudeCode.system,
        [verb]: { ...claudeCode.system[verb], effort },
      },
    })
  }

  function setAzureField<K extends keyof NonNullable<ClaudeCodeBlock["azure"]>>(
    key: K,
    value: NonNullable<ClaudeCodeBlock["azure"]>[K],
  ) {
    const current = claudeCode.azure ?? freshAzureBlock()
    onChange({
      ...claudeCode,
      azure: { ...current, [key]: value },
    })
  }

  function setAzureVerb(verb: Verb, patch: { model?: string; effort?: string }) {
    const current = claudeCode.azure ?? freshAzureBlock()
    onChange({
      ...claudeCode,
      azure: {
        ...current,
        [verb]: { ...current[verb], ...patch },
      },
    })
  }

  async function handleDeleteKey() {
    try {
      await deleteEndpointKey(keyringService)
      setKeyStatus({ kind: "unset" })
      setKeyError(null)
    } catch (err) {
      setKeyError(formatError(err))
    }
  }

  const azure = claudeCode.azure ?? freshAzureBlock()
  const systemActive = claudeCode.active === "system"
  const azureActive = claudeCode.active === "azure"

  return (
    <section
      data-testid="endpoint-section"
      className="col-span-2 flex flex-col gap-3 rounded border border-border bg-bg-secondary/40 p-3"
    >
      <header className="flex items-center justify-between">
        <span className="font-medium text-fg">
          {t("settings.endpoint.claude.heading")}
        </span>
        <fieldset
          role="radiogroup"
          aria-label={t("settings.endpoint.activeProfileAria")}
          data-testid="active-profile-radio"
          className="flex items-center gap-3"
        >
          <ProfileRadio
            label={t("settings.endpoint.profile.system")}
            value="system"
            checked={systemActive}
            onSelect={setActive}
          />
          <ProfileRadio
            label={t("settings.endpoint.profile.azure")}
            value="azure"
            checked={azureActive}
            onSelect={setActive}
          />
        </fieldset>
      </header>

      <ProfileBlock
        title={t("settings.endpoint.profile.systemTitle")}
        inactiveLabel={t("settings.endpoint.profile.inactiveLabel")}
        testId="system-profile"
        active={systemActive}
        expanded={systemExpanded}
        onToggleExpand={() => setSystemExpanded((v) => !v)}
      >
        {VERBS.map((verb) => {
          const effortField = `claude_code.system.${verb}.effort`
          const effortInvalid = hasError(effortField)
          return (
            <VerbRow key={verb} verb={verb} label={t(`settings.endpoint.verb.${verb}`)}>
              <Input
                data-testid={`system-model-${verb}`}
                list="claude-system-model-suggestions"
                className="w-[140px]"
                placeholder={t("settings.endpoint.placeholder.claudeModel")}
                value={claudeCode.system[verb].model}
                onChange={(e) => setSystemModel(verb, e.target.value)}
              />
              <Select
                value={claudeCode.system[verb].effort}
                onValueChange={(v) => setSystemEffort(verb, v)}
              >
                <SelectTrigger
                  className={`w-[100px] ${
                    effortInvalid ? "border-error focus-visible:ring-error" : ""
                  }`}
                  data-testid={`system-effort-${verb}`}
                  aria-invalid={effortInvalid || undefined}
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {SYSTEM_EFFORTS.map((e) => (
                    <SelectItem key={e} value={e}>
                      {e}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </VerbRow>
          )
        })}
        <div
          data-testid="endpoint-chat-row"
          className="flex items-center gap-2 text-fg-tertiary"
        >
          <span className="w-[56px] font-mono text-meta">
            {t("settings.fields.endpointChat.label")}
          </span>
          <span className="font-mono text-meta">
            {t("settings.fields.endpointChat.inherits", {
              model: claudeCode.system.query.model,
              effort: claudeCode.system.query.effort,
            })}
          </span>
        </div>
      </ProfileBlock>

      <ProfileBlock
        title={t("settings.endpoint.profile.azureTitle")}
        inactiveLabel={t("settings.endpoint.profile.inactiveLabel")}
        testId="azure-profile"
        active={azureActive}
        expanded={azureExpanded}
        onToggleExpand={() => setAzureExpanded((v) => !v)}
      >
        <Field label={t("settings.endpoint.field.baseUrl")}>
          <Input
            data-testid="azure-base-url"
            className={`w-full ${
              hasError("claude_code.azure.base_url")
                ? "border-error focus-visible:ring-error"
                : ""
            }`}
            aria-invalid={hasError("claude_code.azure.base_url") || undefined}
            value={azure.base_url}
            placeholder={t("settings.endpoint.placeholder.azureBaseUrlClaude")}
            onChange={(e) => setAzureField("base_url", e.target.value)}
          />
        </Field>
        <Field label={t("settings.endpoint.field.keyringService")}>
          <Input
            data-testid="azure-keyring-service"
            className={`w-full ${
              hasError("claude_code.azure.keyring_service")
                ? "border-error focus-visible:ring-error"
                : ""
            }`}
            aria-invalid={
              hasError("claude_code.azure.keyring_service") || undefined
            }
            value={azure.keyring_service}
            onChange={(e) => setAzureField("keyring_service", e.target.value)}
          />
        </Field>
        <Field label={t("settings.endpoint.field.apiKey")}>
          <div className="flex items-center gap-2">
            <span
              data-testid="azure-key-status"
              className="rounded-full border border-border bg-bg px-2 py-px font-mono text-micro text-fg-secondary"
            >
              {keyStatus?.kind === "set"
                ? t("settings.endpoint.keyStatus.set")
                : keyStatus?.kind === "unset"
                  ? t("settings.endpoint.keyStatus.unset")
                  : t("settings.endpoint.keyStatus.unknown")}
            </span>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              data-testid="azure-key-set"
              onClick={() => setSetKeyOpen(true)}
            >
              {t("settings.endpoint.keySetNew")}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              data-testid="azure-key-delete"
              disabled={keyStatus?.kind !== "set"}
              onClick={() => void handleDeleteKey()}
            >
              {t("settings.endpoint.keyDelete")}
            </Button>
            {keyError && (
              <span className="text-xs text-error" data-testid="azure-key-error">
                {keyError}
              </span>
            )}
          </div>
        </Field>
        {VERBS.map((verb) => {
          const modelField = `claude_code.azure.${verb}.model`
          const effortField = `claude_code.azure.${verb}.effort`
          const modelInvalid = hasError(modelField)
          const effortInvalid = hasError(effortField)
          return (
            <VerbRow key={verb} verb={verb} label={t(`settings.endpoint.verb.${verb}`)}>
              <Input
                data-testid={`azure-deployment-${verb}`}
                className={`w-[200px] ${
                  modelInvalid ? "border-error focus-visible:ring-error" : ""
                }`}
                aria-invalid={modelInvalid || undefined}
                placeholder={t("settings.endpoint.placeholder.deploymentName")}
                value={azure[verb].model}
                onChange={(e) => setAzureVerb(verb, { model: e.target.value })}
              />
              <Select
                value={azure[verb].effort}
                onValueChange={(v) => setAzureVerb(verb, { effort: v })}
              >
                <SelectTrigger
                  className={`w-[100px] ${
                    effortInvalid ? "border-error focus-visible:ring-error" : ""
                  }`}
                  data-testid={`azure-effort-${verb}`}
                  aria-invalid={effortInvalid || undefined}
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {SYSTEM_EFFORTS.map((e) => (
                    <SelectItem key={e} value={e}>
                      {e}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </VerbRow>
          )
        })}
        {errors.length > 0 && (
          <div
            data-testid="endpoint-validation-summary"
            role="alert"
            className="rounded border border-error/40 bg-error/10 px-2 py-1 text-meta text-error"
          >
            <div className="font-medium">
              {t("settings.endpoint.validationSummaryHeading")}
            </div>
            <ul className="ml-3 list-disc">
              {errors.map((e) => (
                <li key={e.field}>{t(e.key, e.vars)}</li>
              ))}
            </ul>
          </div>
        )}
      </ProfileBlock>

      {/* Combobox suggestions for system model — free-text input, these are
         only quick-picks (a new Claude model can be typed directly). */}
      <datalist id="claude-system-model-suggestions">
        {SYSTEM_MODELS.map((m) => (
          <option key={m} value={m} />
        ))}
      </datalist>

      <SetKeyDialog
        open={setKeyOpen}
        service={keyringService}
        onClose={() => setSetKeyOpen(false)}
        onSuccess={() => {
          setSetKeyOpen(false)
          setKeyStatus({ kind: "set" })
          setKeyError(null)
        }}
      />
    </section>
  )
}

function ProfileRadio({
  label,
  value,
  checked,
  onSelect,
}: {
  label: string
  value: "system" | "azure"
  checked: boolean
  onSelect: (v: "system" | "azure") => void
}) {
  return (
    <label className="flex items-center gap-1 text-xs">
      <input
        type="radio"
        name="endpoint-active-profile"
        value={value}
        checked={checked}
        onChange={() => onSelect(value)}
        data-testid={`active-${value}`}
      />
      <span>{label}</span>
    </label>
  )
}

function ProfileBlock({
  title,
  inactiveLabel,
  testId,
  active,
  expanded,
  onToggleExpand,
  children,
}: {
  title: string
  inactiveLabel: string
  testId: string
  active: boolean
  expanded: boolean
  onToggleExpand: () => void
  children: React.ReactNode
}) {
  return (
    <fieldset
      data-testid={testId}
      data-active={active}
      data-expanded={expanded}
      className={`flex flex-col gap-2 rounded border border-border p-2 text-xs transition-opacity ${
        active ? "opacity-100" : "opacity-60"
      }`}
    >
      <legend className="px-1">
        <button
          type="button"
          onClick={onToggleExpand}
          data-testid={`${testId}-header`}
          aria-expanded={expanded}
          className="flex items-center gap-1 font-medium text-fg-secondary hover:text-fg focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-ring"
        >
          <span aria-hidden="true" className="font-mono text-micro">
            {expanded ? "▾" : "▸"}
          </span>
          <span>{title}</span>
          {active ? null : (
            <span
              className="ml-1 font-mono text-micro text-fg-tertiary"
              data-testid={`${testId}-inactive-label`}
            >
              {inactiveLabel}
            </span>
          )}
        </button>
      </legend>
      {/* Body is always in the DOM so input values persist across
         collapse/expand. We hide it via the `hidden` attribute (CSS
         `display: none`) when collapsed. */}
      <div hidden={!expanded} data-testid={`${testId}-body`}>
        {children}
      </div>
    </fieldset>
  )
}

function VerbRow({
  verb,
  label,
  children,
}: {
  verb: Verb
  label: string
  children: React.ReactNode
}) {
  return (
    <div className="flex items-center gap-2" data-verb={verb}>
      <span className="w-[56px] font-mono text-meta text-fg-tertiary">
        {label}
      </span>
      {children}
    </div>
  )
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-2">
      <span className="w-[120px] font-mono text-meta text-fg-tertiary">
        {label}
      </span>
      <div className="flex-1">{children}</div>
    </div>
  )
}

function freshAzureBlock(): NonNullable<ClaudeCodeBlock["azure"]> {
  return {
    base_url: "",
    keyring_service: DEFAULT_CLAUDE_AZURE_KEYRING_SERVICE,
    goal: { model: "", effort: "high" },
    query: { model: "", effort: "low" },
    fix: { model: "", effort: "medium" },
    verify: { model: "", effort: "high" },
  }
}

function formatError(err: unknown): string {
  if (typeof err === "object" && err && "message" in err) {
    const m = (err as { message?: unknown }).message
    if (typeof m === "string") return m
  }
  return String(err)
}
