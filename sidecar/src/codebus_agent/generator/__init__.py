"""Module 5 Generator package — per-station markdown teaching pipeline.

Backs `openspec/changes/module-5-generator-p0/specs/module-5-generator/spec.md`.

Public API (import directly from submodules to avoid triggering the
`codebus_agent.api` import chain via this package's eager re-exports —
see `module-5-generator-p0` Section 17 verification commit notes):

    from codebus_agent.generator.runner import run_generator
    from codebus_agent.generator.types import GeneratorResult, GeneratorOptions

Internal layout (see spec Requirements):
    - ``types``         — Pydantic schemas (Station / Frontmatter / GeneratorResult ...)
    - ``runner``        — orchestrator: per-station loop + MOC + route.json + log
    - ``station``       — per-station LLM call + retry + Sanitizer Pass 1 + write file
    - ``validator``     — D-029 component rules (Checkpoint / Quiz / CodeRef / length / code-block)
    - ``stable_id``     — ``s{NN}-{slug}`` generator + collision handling
    - ``frontmatter``   — D-029 schema_version 1 YAML renderer
    - ``moc``           — ``tutorial.md`` MOC assembler (pure index, standard markdown links)
    - ``route``         — ``route.json`` writer (D-029 §八 schema)
    - ``log``           — ``generator_log.jsonl`` operational log writer
    - ``prompts``       — ``STATION_SYSTEM_INTERACTIVE`` / ``STATION_SYSTEM_PLAIN`` + version
"""
from __future__ import annotations
