"""Provider pool config schema loader.

Backs SHALL clauses in
openspec/changes/provider-settings-and-onboarding/specs/sidecar-runtime/spec.md
  Requirement: Config schema supports provider pool with role bindings
    Scenario: New schema accepted
    Scenario: Legacy schema converted with deprecation warning
    Scenario: Binding referencing unknown provider rejected
    Scenario: Embed binding to chat-typed provider rejected

Plus the related Requirement (Sidecar accepts provider config mutation
endpoints) clause: `pii.mode == "llm"` requires a `provider_id` that
resolves to a PII-allowlisted provider type.

Two schema shapes:

New (D-033 B canonical):
    [[llm.providers]]
    id = "openai-default"
    type = "openai_chat"
    model = "gpt-4o-mini"
    base_url = "https://api.openai.com/v1"

    [llm.bindings]
    reasoning = "openai-default"
    judge = "openai-default"
    chat = "openai-default"
    embed = "openai-embed-3-small"

    [llm.pii]
    mode = "rule"          # or "llm"
    provider_id = ""       # required when mode == "llm"

Legacy (M1-era flat):
    [llm.roles.reasoning]
    provider_id = "openai-default"
    type = "openai_chat"
    model = "gpt-4o-mini"
    base_url = "https://api.openai.com/v1"

The loader converts legacy → new in-memory and emits exactly one
DeprecationWarning per process start.
"""
from __future__ import annotations

import warnings
from dataclasses import dataclass, field
from typing import Any, Literal

INVALID_PROVIDER_BINDING = "INVALID_PROVIDER_BINDING"
INVALID_PROVIDER_TYPE = "INVALID_PROVIDER_TYPE"
INVALID_PII_PROVIDER = "INVALID_PII_PROVIDER"


_EMBEDDING_TYPES: frozenset[str] = frozenset({"openai_embedding"})
_CHAT_TYPES: frozenset[str] = frozenset({"openai_chat"})

# PII allowlist — P0 has none; LocalLLMPIIProvider lands in P1+.
_PII_ALLOWED_TYPES: frozenset[str] = frozenset()


PiiMode = Literal["rule", "llm"]


class ProviderPoolConfigError(ValueError):
    """Raised when the provider pool config is malformed.

    `code` is one of the module-level constants
    (`INVALID_PROVIDER_BINDING` / `INVALID_PROVIDER_TYPE` /
    `INVALID_PII_PROVIDER`) so callers can branch on the failure mode
    without parsing the message.
    """

    def __init__(self, code: str, message: str) -> None:
        super().__init__(f"{code}: {message}")
        self.code = code


@dataclass(frozen=True)
class ProviderSpec:
    """One entry in `llm.providers[]`.

    `api_key` lives in `app.state.provider_keys`, NOT here. The schema
    forbids it on disk per D-033 B Decision 1 invariant.
    """

    id: str
    type: str
    model: str
    base_url: str


@dataclass(frozen=True)
class ProviderPoolSnapshot:
    """In-memory representation of `[llm]` after validation."""

    providers: tuple[ProviderSpec, ...]
    bindings: dict[str, str] = field(default_factory=dict)
    pii_mode: PiiMode = "rule"
    pii_provider_id: str | None = None


def load_provider_pool(raw: dict[str, Any]) -> ProviderPoolSnapshot:
    """Parse and validate the `[llm]` section of a sidecar config dict.

    `raw` is the parsed TOML dict (or any equivalent mapping). The
    function returns an immutable `ProviderPoolSnapshot` on success and
    raises `ProviderPoolConfigError` with one of the three canonical
    codes on the first validation failure.
    """
    llm = dict(raw.get("llm", {}) or {})

    providers_raw = llm.get("providers")
    if providers_raw is None and "roles" in llm:
        providers_raw, bindings_raw = _convert_legacy_roles(llm["roles"])
    else:
        bindings_raw = dict(llm.get("bindings", {}) or {})

    providers = tuple(
        ProviderSpec(
            id=str(entry["id"]),
            type=str(entry["type"]),
            model=str(entry["model"]),
            base_url=str(entry["base_url"]),
        )
        for entry in (providers_raw or [])
    )
    by_id = {p.id: p for p in providers}

    _validate_bindings(bindings_raw, by_id)

    pii_raw = dict(llm.get("pii", {}) or {})
    pii_mode = pii_raw.get("mode", "rule")
    if pii_mode not in ("rule", "llm"):
        raise ProviderPoolConfigError(
            INVALID_PII_PROVIDER,
            f"llm.pii.mode must be 'rule' or 'llm'; got {pii_mode!r}",
        )
    pii_provider_id = pii_raw.get("provider_id") or None
    if pii_mode == "llm":
        if not pii_provider_id:
            raise ProviderPoolConfigError(
                INVALID_PII_PROVIDER,
                "llm.pii.mode == 'llm' requires llm.pii.provider_id",
            )
        provider = by_id.get(pii_provider_id)
        if provider is None:
            raise ProviderPoolConfigError(
                INVALID_PII_PROVIDER,
                f"llm.pii.provider_id {pii_provider_id!r} does not match any "
                "entry in llm.providers[]",
            )
        if provider.type not in _PII_ALLOWED_TYPES:
            raise ProviderPoolConfigError(
                INVALID_PII_PROVIDER,
                f"provider {pii_provider_id!r} type {provider.type!r} is not "
                "in the PII allowlist (P0: empty — LocalLLMPIIProvider lands in P1+)",
            )

    return ProviderPoolSnapshot(
        providers=providers,
        bindings=dict(bindings_raw),
        pii_mode=pii_mode,
        pii_provider_id=pii_provider_id,
    )


def _validate_bindings(
    bindings: dict[str, Any], providers_by_id: dict[str, ProviderSpec]
) -> None:
    """Enforce binding existence + embed type-match invariants."""
    for role, provider_id in bindings.items():
        if provider_id not in providers_by_id:
            raise ProviderPoolConfigError(
                INVALID_PROVIDER_BINDING,
                f"role {role!r} bound to unknown provider id {provider_id!r}",
            )
        if role == "embed":
            provider = providers_by_id[provider_id]
            if provider.type not in _EMBEDDING_TYPES:
                raise ProviderPoolConfigError(
                    INVALID_PROVIDER_TYPE,
                    f"role 'embed' bound to provider {provider_id!r} of type "
                    f"{provider.type!r}; expected an embedding-shaped type "
                    f"({sorted(_EMBEDDING_TYPES)})",
                )


def _convert_legacy_roles(
    roles: dict[str, Any],
) -> tuple[list[dict[str, Any]], dict[str, str]]:
    """Convert `[llm.roles.<role>]` legacy shape into `(providers, bindings)`.

    Each role entry already names a `provider_id`; reuse it across roles
    when multiple roles point at the same id.
    """
    warnings.warn(
        "config: [llm.roles] is deprecated — migrate to [[llm.providers]] + "
        "[llm.bindings]; the legacy shape will be removed in a future release",
        DeprecationWarning,
        stacklevel=3,
    )
    providers: dict[str, dict[str, Any]] = {}
    bindings: dict[str, str] = {}
    for role, body in roles.items():
        provider_id = str(body["provider_id"])
        if provider_id not in providers:
            providers[provider_id] = {
                "id": provider_id,
                "type": str(body["type"]),
                "model": str(body["model"]),
                "base_url": str(body["base_url"]),
            }
        bindings[role] = provider_id
    return list(providers.values()), bindings
