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
  type SystemModel,
  DEFAULT_AZURE_KEYRING_SERVICE,
  SYSTEM_MODELS,
  deleteEndpointKey,
  getEndpointKey,
} from "@/lib/ipc"

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

const VERBS = ["goal", "query", "fix"] as const
type Verb = (typeof VERBS)[number]

export function EndpointSection({
  claudeCode,
  onChange,
  errors = [],
}: EndpointSectionProps) {
  const hasError = (field: string) => errors.some((e) => e.field === field)
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
      const status = await getEndpointKey("azure")
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

  function setSystemModel(verb: Verb, model: SystemModel) {
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
      await deleteEndpointKey("azure")
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
        <span className="font-medium text-fg">Claude Code endpoint settings</span>
        <fieldset
          role="radiogroup"
          aria-label="Active endpoint profile"
          data-testid="active-profile-radio"
          className="flex items-center gap-3"
        >
          <ProfileRadio
            label="System"
            value="system"
            checked={systemActive}
            onSelect={setActive}
          />
          <ProfileRadio
            label="Azure"
            value="azure"
            checked={azureActive}
            onSelect={setActive}
          />
        </fieldset>
      </header>

      <ProfileBlock
        title="System Profile"
        testId="system-profile"
        active={systemActive}
        expanded={systemExpanded}
        onToggleExpand={() => setSystemExpanded((v) => !v)}
      >
        {VERBS.map((verb) => (
          <VerbRow key={verb} verb={verb}>
            <Select
              value={claudeCode.system[verb].model}
              onValueChange={(v) => setSystemModel(verb, v as SystemModel)}
            >
              <SelectTrigger
                className="w-[140px]"
                data-testid={`system-model-${verb}`}
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {SYSTEM_MODELS.map((m) => (
                  <SelectItem key={m} value={m}>
                    {m}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Input
              data-testid={`system-effort-${verb}`}
              className="w-[100px]"
              value={claudeCode.system[verb].effort}
              onChange={(e) => setSystemEffort(verb, e.target.value)}
            />
          </VerbRow>
        ))}
      </ProfileBlock>

      <ProfileBlock
        title="Azure Profile"
        testId="azure-profile"
        active={azureActive}
        expanded={azureExpanded}
        onToggleExpand={() => setAzureExpanded((v) => !v)}
      >
        <Field label="base_url">
          <Input
            data-testid="azure-base-url"
            className={`w-full ${
              hasError("claude_code.azure.base_url")
                ? "border-error focus-visible:ring-error"
                : ""
            }`}
            aria-invalid={hasError("claude_code.azure.base_url") || undefined}
            value={azure.base_url}
            placeholder="https://<resource>.cognitiveservices.azure.com/anthropic"
            onChange={(e) => setAzureField("base_url", e.target.value)}
          />
        </Field>
        <Field label="keyring_service">
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
        <Field label="API key">
          <div className="flex items-center gap-2">
            <span
              data-testid="azure-key-status"
              className="rounded-full border border-border bg-bg px-2 py-px font-mono text-[10px] text-fg-secondary"
            >
              {keyStatus?.kind === "set"
                ? "Set"
                : keyStatus?.kind === "unset"
                  ? "Unset"
                  : "—"}
            </span>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              data-testid="azure-key-set"
              onClick={() => setSetKeyOpen(true)}
            >
              Set new…
            </Button>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              data-testid="azure-key-delete"
              disabled={keyStatus?.kind !== "set"}
              onClick={() => void handleDeleteKey()}
            >
              Delete
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
          const invalid = hasError(modelField)
          return (
            <VerbRow key={verb} verb={verb}>
              <Input
                data-testid={`azure-deployment-${verb}`}
                className={`w-[200px] ${
                  invalid ? "border-error focus-visible:ring-error" : ""
                }`}
                aria-invalid={invalid || undefined}
                placeholder="<deployment name>"
                value={azure[verb].model}
                onChange={(e) => setAzureVerb(verb, { model: e.target.value })}
              />
              <Input
                data-testid={`azure-effort-${verb}`}
                className="w-[100px]"
                value={azure[verb].effort}
                onChange={(e) => setAzureVerb(verb, { effort: e.target.value })}
              />
            </VerbRow>
          )
        })}
        {errors.length > 0 && (
          <div
            data-testid="endpoint-validation-summary"
            role="alert"
            className="rounded border border-error/40 bg-error/10 px-2 py-1 text-[11px] text-error"
          >
            <div className="font-medium">
              Endpoint configuration is incomplete:
            </div>
            <ul className="ml-3 list-disc">
              {errors.map((e) => (
                <li key={e.field}>{e.message}</li>
              ))}
            </ul>
          </div>
        )}
      </ProfileBlock>

      <SetKeyDialog
        open={setKeyOpen}
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
  testId,
  active,
  expanded,
  onToggleExpand,
  children,
}: {
  title: string
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
          <span aria-hidden="true" className="font-mono text-[10px]">
            {expanded ? "▾" : "▸"}
          </span>
          <span>{title}</span>
          {active ? null : (
            <span
              className="ml-1 font-mono text-[10px] text-fg-tertiary"
              data-testid={`${testId}-inactive-label`}
            >
              (inactive)
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

function VerbRow({ verb, children }: { verb: Verb; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-2">
      <span className="w-[56px] font-mono text-[11px] text-fg-tertiary">
        {verb}
      </span>
      {children}
    </div>
  )
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-2">
      <span className="w-[120px] font-mono text-[11px] text-fg-tertiary">
        {label}
      </span>
      <div className="flex-1">{children}</div>
    </div>
  )
}

function freshAzureBlock(): NonNullable<ClaudeCodeBlock["azure"]> {
  return {
    base_url: "",
    keyring_service: DEFAULT_AZURE_KEYRING_SERVICE,
    goal: { model: "", effort: "high" },
    query: { model: "", effort: "low" },
    fix: { model: "", effort: "medium" },
  }
}

function formatError(err: unknown): string {
  if (typeof err === "object" && err && "message" in err) {
    const m = (err as { message?: unknown }).message
    if (typeof m === "string") return m
  }
  return String(err)
}
