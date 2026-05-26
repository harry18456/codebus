import { useEffect, useState } from "react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  type ClaudeCodeValidationError,
  type CodexBlock,
  type KeyStatus,
  DEFAULT_CODEX_AZURE_KEYRING_SERVICE,
  deleteEndpointKey,
  getEndpointKey,
} from "@/lib/ipc"

import { useT } from "@/i18n/useT"

import { SetKeyDialog } from "./SetKeyDialog"

/**
 * Codex provider endpoint editor — the codex registry entry's editor
 * component. Mirrors the claude `EndpointSection` accordion/active-radio
 * shape, with two codex-specific differences: `model` is a free-text input
 * (codex model names are arbitrary, not a closed enum) and the azure profile
 * carries an `api_version` field (Responses API). `effort` is also free text.
 */
export interface CodexEndpointSectionProps {
  block: CodexBlock
  onChange: (next: CodexBlock) => void
  errors?: ClaudeCodeValidationError[]
}

const VERBS = ["goal", "query", "fix", "verify"] as const
type Verb = (typeof VERBS)[number]

// Combobox suggestions — sourced from `codex /model` (2026-05). Inputs stay
// free-text: Azure deployment names are user-defined and codex models evolve,
// so these are convenience hints, NOT a closed enum.
const CODEX_MODEL_SUGGESTIONS = [
  "gpt-5.5",
  "gpt-5.4",
  "gpt-5.4-mini",
  "gpt-5.3-codex",
  "gpt-5.2",
] as const
// `model_reasoning_effort` values (codex `/model` shows Low/Medium/High/Extra
// high; `xhigh` is the assumed TOML token for "Extra high" — free-text covers
// it if the real token differs).
const CODEX_EFFORT_SUGGESTIONS = ["low", "medium", "high", "xhigh"] as const
const MODEL_LIST_ID = "codex-model-suggestions"
const EFFORT_LIST_ID = "codex-effort-suggestions"

export function CodexEndpointSection({
  block,
  onChange,
  errors = [],
}: CodexEndpointSectionProps) {
  const t = useT()
  const hasError = (field: string) => errors.some((e) => e.field === field)
  // Codex azure key lives under the codex profile's `keyring_service`
  // (default `codebus-codex-azure`) — distinct from claude's entry.
  const keyringService =
    block.azure?.keyring_service?.trim() || DEFAULT_CODEX_AZURE_KEYRING_SERVICE
  const [keyStatus, setKeyStatus] = useState<KeyStatus | null>(null)
  const [keyError, setKeyError] = useState<string | null>(null)
  const [setKeyOpen, setSetKeyOpen] = useState(false)
  const [systemExpanded, setSystemExpanded] = useState(block.active === "system")
  const [azureExpanded, setAzureExpanded] = useState(block.active === "azure")

  useEffect(() => {
    void refreshKeyStatus()
  }, [])

  useEffect(() => {
    if (block.active === "system") {
      setSystemExpanded(true)
      setAzureExpanded(false)
    } else {
      setSystemExpanded(false)
      setAzureExpanded(true)
    }
  }, [block.active])

  async function refreshKeyStatus() {
    try {
      setKeyStatus(await getEndpointKey(keyringService))
      setKeyError(null)
    } catch (err) {
      setKeyStatus(null)
      setKeyError(formatError(err))
    }
  }

  function setActive(next: "system" | "azure") {
    onChange({ ...block, active: next })
  }

  function setSystemVerb(verb: Verb, patch: { model?: string; effort?: string }) {
    onChange({
      ...block,
      system: { ...block.system, [verb]: { ...block.system[verb], ...patch } },
    })
  }

  function setAzureField<K extends keyof NonNullable<CodexBlock["azure"]>>(
    key: K,
    value: NonNullable<CodexBlock["azure"]>[K],
  ) {
    const current = block.azure ?? freshAzure()
    onChange({ ...block, azure: { ...current, [key]: value } })
  }

  function setAzureVerb(verb: Verb, patch: { model?: string; effort?: string }) {
    const current = block.azure ?? freshAzure()
    onChange({
      ...block,
      azure: { ...current, [verb]: { ...current[verb], ...patch } },
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

  const azure = block.azure ?? freshAzure()
  const systemActive = block.active === "system"
  const azureActive = block.active === "azure"

  return (
    <section
      data-testid="codex-endpoint-section"
      className="col-span-2 flex flex-col gap-3 rounded border border-border bg-bg-secondary/40 p-3"
    >
      <header className="flex items-center justify-between">
        <span className="font-medium text-fg">
          {t("settings.endpoint.codex.heading")}
        </span>
        <fieldset
          role="radiogroup"
          aria-label={t("settings.endpoint.activeProfileAriaCodex")}
          data-testid="codex-active-profile-radio"
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
        testId="codex-system-profile"
        active={systemActive}
        expanded={systemExpanded}
        onToggleExpand={() => setSystemExpanded((v) => !v)}
      >
        {VERBS.map((verb) => {
          const modelInvalid = hasError(`codex.system.${verb}.model`)
          return (
            <VerbRow key={verb} verb={verb} label={t(`settings.endpoint.verb.${verb}`)}>
              <Input
                data-testid={`codex-system-model-${verb}`}
                list={MODEL_LIST_ID}
                className={`w-[160px] ${modelInvalid ? "border-error focus-visible:ring-error" : ""}`}
                aria-invalid={modelInvalid || undefined}
                placeholder={t("settings.endpoint.placeholder.codexModel")}
                value={block.system[verb].model}
                onChange={(e) => setSystemVerb(verb, { model: e.target.value })}
              />
              <Input
                data-testid={`codex-system-effort-${verb}`}
                list={EFFORT_LIST_ID}
                className="w-[100px]"
                placeholder={t("settings.endpoint.placeholder.codexEffort")}
                value={block.system[verb].effort}
                onChange={(e) => setSystemVerb(verb, { effort: e.target.value })}
              />
            </VerbRow>
          )
        })}
        <div
          data-testid="codex-endpoint-chat-row"
          className="flex items-center gap-2 text-fg-tertiary"
        >
          <span className="w-[56px] font-mono text-meta">
            {t("settings.fields.endpointChat.label")}
          </span>
          <span className="font-mono text-meta">
            {t("settings.fields.endpointChat.inherits", {
              model: block.system.query.model,
              effort: block.system.query.effort,
            })}
          </span>
        </div>
      </ProfileBlock>

      <ProfileBlock
        title={t("settings.endpoint.profile.azureTitle")}
        inactiveLabel={t("settings.endpoint.profile.inactiveLabel")}
        testId="codex-azure-profile"
        active={azureActive}
        expanded={azureExpanded}
        onToggleExpand={() => setAzureExpanded((v) => !v)}
      >
        <Field label={t("settings.endpoint.field.baseUrl")}>
          <Input
            data-testid="codex-azure-base-url"
            className={`w-full ${hasError("codex.azure.base_url") ? "border-error focus-visible:ring-error" : ""}`}
            aria-invalid={hasError("codex.azure.base_url") || undefined}
            value={azure.base_url}
            placeholder={t("settings.endpoint.placeholder.azureBaseUrlCodex")}
            onChange={(e) => setAzureField("base_url", e.target.value.trim())}
          />
        </Field>
        <Field label={t("settings.endpoint.field.apiVersion")}>
          <Input
            data-testid="codex-azure-api-version"
            className={`w-full ${hasError("codex.azure.api_version") ? "border-error focus-visible:ring-error" : ""}`}
            aria-invalid={hasError("codex.azure.api_version") || undefined}
            value={azure.api_version}
            placeholder={t("settings.endpoint.placeholder.apiVersion")}
            onChange={(e) => setAzureField("api_version", e.target.value.trim())}
          />
        </Field>
        <Field label={t("settings.endpoint.field.keyringService")}>
          <Input
            data-testid="codex-azure-keyring-service"
            className={`w-full ${hasError("codex.azure.keyring_service") ? "border-error focus-visible:ring-error" : ""}`}
            aria-invalid={hasError("codex.azure.keyring_service") || undefined}
            value={azure.keyring_service}
            onChange={(e) => setAzureField("keyring_service", e.target.value)}
          />
        </Field>
        <Field label={t("settings.endpoint.field.apiKey")}>
          <div className="flex items-center gap-2">
            <span
              data-testid="codex-azure-key-status"
              className="rounded-full border border-border bg-bg px-2 py-px font-mono text-micro text-fg-secondary"
            >
              {keyStatus?.kind === "set"
                ? t("settings.endpoint.keyStatus.set")
                : keyStatus?.kind === "unset"
                  ? t("settings.endpoint.keyStatus.unset")
                  : t("settings.endpoint.keyStatus.unknown")}
            </span>
            <Button type="button" size="sm" variant="secondary" data-testid="codex-azure-key-set" onClick={() => setSetKeyOpen(true)}>
              {t("settings.endpoint.keySetNew")}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              data-testid="codex-azure-key-delete"
              disabled={keyStatus?.kind !== "set"}
              onClick={() => void handleDeleteKey()}
            >
              {t("settings.endpoint.keyDelete")}
            </Button>
            {keyError && (
              <span className="text-xs text-error" data-testid="codex-azure-key-error">
                {keyError}
              </span>
            )}
          </div>
        </Field>
        {VERBS.map((verb) => {
          const modelInvalid = hasError(`codex.azure.${verb}.model`)
          return (
            <VerbRow key={verb} verb={verb} label={t(`settings.endpoint.verb.${verb}`)}>
              <Input
                data-testid={`codex-azure-deployment-${verb}`}
                list={MODEL_LIST_ID}
                className={`w-[200px] ${modelInvalid ? "border-error focus-visible:ring-error" : ""}`}
                aria-invalid={modelInvalid || undefined}
                placeholder={t("settings.endpoint.placeholder.deploymentName")}
                value={azure[verb].model}
                onChange={(e) => setAzureVerb(verb, { model: e.target.value })}
              />
              <Input
                data-testid={`codex-azure-effort-${verb}`}
                list={EFFORT_LIST_ID}
                className="w-[100px]"
                placeholder={t("settings.endpoint.placeholder.codexEffort")}
                value={azure[verb].effort}
                onChange={(e) => setAzureVerb(verb, { effort: e.target.value })}
              />
            </VerbRow>
          )
        })}
        <div
          data-testid="codex-azure-endpoint-chat-row"
          className="flex items-center gap-2 text-fg-tertiary"
        >
          <span className="w-[56px] font-mono text-meta">
            {t("settings.fields.endpointChat.label")}
          </span>
          <span className="font-mono text-meta">
            {t("settings.fields.endpointChat.inherits", {
              model: azure.query.model,
              effort: azure.query.effort,
            })}
          </span>
        </div>
        {errors.length > 0 && (
          <div
            data-testid="codex-endpoint-validation-summary"
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

      {/* Combobox suggestion lists — shared by all model / effort inputs.
         Inputs remain free-text; these only surface quick-pick options. */}
      <datalist id={MODEL_LIST_ID}>
        {CODEX_MODEL_SUGGESTIONS.map((m) => (
          <option key={m} value={m} />
        ))}
      </datalist>
      <datalist id={EFFORT_LIST_ID}>
        {CODEX_EFFORT_SUGGESTIONS.map((e) => (
          <option key={e} value={e} />
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
        name="codex-endpoint-active-profile"
        value={value}
        checked={checked}
        onChange={() => onSelect(value)}
        data-testid={`codex-active-${value}`}
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
      className={`flex flex-col gap-2 rounded border border-border p-2 text-xs transition-opacity ${active ? "opacity-100" : "opacity-60"}`}
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
            <span className="ml-1 font-mono text-micro text-fg-tertiary" data-testid={`${testId}-inactive-label`}>
              {inactiveLabel}
            </span>
          )}
        </button>
      </legend>
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
      <span className="w-[120px] font-mono text-meta text-fg-tertiary">{label}</span>
      <div className="flex-1">{children}</div>
    </div>
  )
}

function freshAzure(): NonNullable<CodexBlock["azure"]> {
  return {
    base_url: "",
    api_version: "",
    keyring_service: DEFAULT_CODEX_AZURE_KEYRING_SERVICE,
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
