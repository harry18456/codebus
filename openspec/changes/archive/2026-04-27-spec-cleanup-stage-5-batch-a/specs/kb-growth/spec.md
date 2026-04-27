## MODIFIED Requirements

### Requirement: kb_growth.jsonl path constant lives alongside other audit filenames

The sidecar SHALL declare the module-level constant `_KB_GROWTH_FILENAME = "kb_growth.jsonl"` at exactly one canonical location: `codebus_agent/_audit_paths.py` (the **package-root leaf module**, established by `audit-path-unification` archive 2026-04-25 to break a three-way circular import among `api/__init__.py` ↔ `api/generate.py` ↔ `generator/runner.py`). The legacy location `codebus_agent/api/_audit_paths.py` MUST remain as a backward-compat re-export shim that imports the seven path constants from `codebus_agent._audit_paths` and re-exports them under the same names; existing call sites MAY import from either path and MUST receive the same Python string object via identity (`is`) check.

The constant MUST be importable as `from codebus_agent._audit_paths import _KB_GROWTH_FILENAME` (canonical) or `from codebus_agent.api._audit_paths import _KB_GROWTH_FILENAME` (backward-compat). Any factory that constructs a `KBGrowthLogger` MUST resolve its path using this constant rather than embedding the literal string `"kb_growth.jsonl"`.

#### Scenario: Constant exported from canonical leaf module

- **WHEN** `from codebus_agent._audit_paths import _KB_GROWTH_FILENAME` is executed
- **THEN** the import MUST succeed and the value MUST equal the literal string `"kb_growth.jsonl"`

#### Scenario: Backward-compat shim re-exports same object

- **WHEN** `from codebus_agent.api._audit_paths import _KB_GROWTH_FILENAME` is executed
- **THEN** the import MUST succeed
- **AND** the imported value MUST be the same Python string object as the one imported from `codebus_agent._audit_paths` (identity check via `is`)

#### Scenario: No literal "kb_growth.jsonl" outside the canonical leaf module

- **WHEN** `sidecar/src/codebus_agent/` is grepped for the quoted-string literal `"kb_growth.jsonl"` (the string surrounded by double or single quotes)
- **THEN** the only match MUST be inside `codebus_agent/_audit_paths.py` (the canonical leaf module)
- **AND** `codebus_agent/api/_audit_paths.py` MUST contain only an `import` line for `_KB_GROWTH_FILENAME` and an `__all__` listing — it MUST NOT contain a freshly declared `_KB_GROWTH_FILENAME = "kb_growth.jsonl"` literal of its own
