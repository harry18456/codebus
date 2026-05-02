"""Persistence for the provider pool config (App-level mirror).

`~/.codebus/llm-config.json` mirrors the in-memory
``ProviderPoolSnapshot``. API keys NEVER appear here — they live only
in the OS keyring per the D-033 B trust boundary. This file holds
metadata only (id / type / model / base_url / bindings / pii_mode /
pii_provider_id).

Atomic write: serialize to ``<path>.tmp`` then ``os.replace()`` so a
crash mid-write never leaves a partial JSON behind. Read failures
(file missing / malformed JSON / schema validation error) all fall
back to an empty default + log warn — a corrupt config MUST NOT
brick sidecar boot.

Backs SHALL clauses in
``openspec/changes/phase7-onboarding-polish/specs/keyring-integration/spec.md``
  Requirement: Provider pool persists to disk across sidecar restarts
"""
from __future__ import annotations

import json
import logging
import os
from pathlib import Path
from typing import Any

from codebus_agent.auth import paths as _paths
from codebus_agent.config.provider_pool import (
    ProviderPoolSnapshot,
    load_provider_pool,
)

LLM_CONFIG_SCHEMA_VERSION = 1

logger = logging.getLogger(__name__)

__all__ = [
    "LLM_CONFIG_SCHEMA_VERSION",
    "load_llm_config_or_default",
    "save_llm_config",
]


def _empty_default() -> ProviderPoolSnapshot:
    return ProviderPoolSnapshot(
        providers=(),
        bindings={},
        pii_mode="rule",
        pii_provider_id=None,
    )


def save_llm_config(
    snapshot: ProviderPoolSnapshot, *, path: Path | None = None
) -> None:
    """Atomically persist ``snapshot`` to the App-level config file.

    ``path`` override is for tests; production callers omit it and
    let `auth.paths.llm_config_path()` resolve.
    """
    target = path or _paths.llm_config_path()
    target.parent.mkdir(parents=True, exist_ok=True)
    tmp = target.with_suffix(target.suffix + ".tmp")
    payload: dict[str, Any] = {
        "version": LLM_CONFIG_SCHEMA_VERSION,
        "providers": [
            {
                "id": p.id,
                "type": p.type,
                "model": p.model,
                "base_url": p.base_url,
            }
            for p in snapshot.providers
        ],
        "bindings": dict(snapshot.bindings),
        "pii_mode": snapshot.pii_mode,
        "pii_provider_id": snapshot.pii_provider_id,
    }
    tmp.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    os.replace(tmp, target)


def load_llm_config_or_default(
    *, path: Path | None = None
) -> ProviderPoolSnapshot:
    """Read snapshot from disk, falling back to empty on any failure.

    Failure modes (all → empty default + warn):
      - File missing (first boot, fresh install)
      - JSON parse error (corrupt / truncated)
      - Schema validation error (manually edited to invalid shape)

    Successful read maps the on-disk flat shape into the
    ``[llm]``-keyed structure that ``load_provider_pool()`` expects so
    we reuse the existing validator (binding existence / embed
    type-match / pii allowlist).
    """
    target = path or _paths.llm_config_path()
    if not target.exists():
        return _empty_default()
    try:
        raw = json.loads(target.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError) as e:
        logger.warning(
            "llm-config.json read failed (%s) — falling back to empty",
            type(e).__name__,
        )
        return _empty_default()
    pii_provider_id = raw.get("pii_provider_id") or ""
    raw_llm = {
        "llm": {
            "providers": raw.get("providers", []),
            "bindings": raw.get("bindings", {}),
            "pii": {
                "mode": raw.get("pii_mode", "rule"),
                "provider_id": pii_provider_id,
            },
        }
    }
    try:
        return load_provider_pool(raw_llm)
    except Exception as e:
        logger.warning(
            "llm-config.json schema validation failed (%s) — falling back to empty",
            e,
        )
        return _empty_default()
