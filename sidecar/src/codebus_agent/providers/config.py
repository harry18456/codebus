"""LLM config parsing — role map → `dict[ProviderRole, RoleConfig]`.

Backs SHALL clauses in
openspec/changes/llm-role-routing/specs/llm-provider/spec.md
  Requirement: Config schema declares llm.roles map
    Scenario: Config roles map parses into RoleConfig instances
    Scenario: Config rejects unknown role key

Design llm-role-routing §4: the canonical config shape keeps an
`llm.llm_disabled` kill switch alongside an `llm.roles` map whose
keys are lowercase `ProviderRole` values.  M1's flat
`llm.chat_provider` / `llm.embed_provider` fields are retired.

This module is the schema contract only — an actual config loader
(pydantic-settings, tomli, …) is deferred per llm-role-routing
"Open Questions" and will live in a later change.
"""
from __future__ import annotations

from typing import Any

from .protocol import ProviderRole, RoleConfig


def parse_llm_roles(config: dict[str, Any]) -> dict[ProviderRole, RoleConfig]:
    """Parse `config["llm"]["roles"]` into a role-to-RoleConfig map.

    Missing `llm` or `llm.roles` keys yield an empty map — callers
    decide whether that's a hard error or a "no roles configured"
    state (cf. `llm_disabled` kill switch).

    Raises
    ------
    ValueError
        If any role key is not a valid `ProviderRole` lowercase value.
        The message names the offending key and lists the four valid
        role names.
    TypeError
        If a role payload is missing `provider_id` / `model`, or
        contains fields not accepted by `RoleConfig` — surfaced from
        the dataclass constructor.
    """
    roles_raw = config.get("llm", {}).get("roles", {})
    if not isinstance(roles_raw, dict):
        raise TypeError(
            f"expected llm.roles to be a dict; got {type(roles_raw).__name__}"
        )

    valid_role_names = [r.value for r in ProviderRole]
    parsed: dict[ProviderRole, RoleConfig] = {}
    for key, payload in roles_raw.items():
        try:
            role = ProviderRole(key)
        except ValueError:
            raise ValueError(
                f"unknown role key {key!r} in llm.roles; "
                f"valid roles are: {valid_role_names}"
            ) from None
        if not isinstance(payload, dict):
            raise TypeError(
                f"role {key!r} payload must be a dict; got {type(payload).__name__}"
            )
        parsed[role] = RoleConfig(**payload)
    return parsed


def is_llm_disabled(config: dict[str, Any]) -> bool:
    """Return the `llm.llm_disabled` kill-switch value (default False).

    Kept alongside `parse_llm_roles` so callers have a single import
    surface for the `llm.*` config keys the role-routing schema
    reserves.
    """
    return bool(config.get("llm", {}).get("llm_disabled", False))
