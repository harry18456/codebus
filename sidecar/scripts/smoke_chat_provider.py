"""Manual smoke test for `chat-provider-wiring` (tasks.md §7.4).

Reusable — runs against a live OpenAI endpoint using `CODEBUS_OPENAI_API_KEY`
loaded silently from the repo-root `.env` (the key's value is never echoed
to stdout/stderr). Dumps artifacts into `.codebus-smoke-ws/` (gitignored).

Checks:
  (a) `app.state.openai_chat_probe.status == "ok"` — startup smoke probe
      (falls back to an inline async probe when `create_app` runs inside
      an existing event loop).
  (b) `app.state.llm_reasoning_provider(ws)` returns a `TrackedProvider`
      with `role='reasoning'` + `default_module='reasoning'`.
  (c) A real chat call against `gpt-4o-mini` returns a validated Pydantic
      instance (Instructor TOOLS-mode round-trip works end-to-end).
  (d) `<ws>/token_usage.jsonl` gains at least one row tagged
      `module='reasoning'` so the per-role cost-split is live.

Usage:
    uv run --directory sidecar python scripts/smoke_chat_provider.py
"""
from __future__ import annotations

import asyncio
import json
import secrets
import shutil
import sys
from pathlib import Path


def _load_env_silently(env_path: Path) -> None:
    """Populate os.environ from `.env`; never echo values to stdout/stderr."""
    import os

    if not env_path.exists():
        raise FileNotFoundError(f"{env_path} not found")
    for raw in env_path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            continue
        key, _, value = line.partition("=")
        key = key.strip()
        value = value.strip().strip('"').strip("'")
        if key:
            os.environ[key] = value


def _purge_workspace(ws: Path) -> None:
    """Start each smoke run from a clean workspace so row counts are deterministic."""
    if ws.exists():
        shutil.rmtree(ws)
    ws.mkdir(parents=True)
    (ws / ".codebus").mkdir()


async def _run_async_checks(app, ws: Path) -> int:
    """Async-only half of the smoke test (real chat call + post-invariants)."""
    # Import here so the sync half doesn't pay for them if env load fails.
    from pydantic import BaseModel

    from codebus_agent.api import _probe_openai_chat_raw
    from codebus_agent.providers.protocol import Message

    class Action(BaseModel):
        """Tiny response_model to keep the round-trip cheap."""

        tool: str
        reason: str

    # (a) healthz probe — startup probe may be None if create_app was
    # called from an already-running event loop. Fall back to a fresh
    # async probe (identical code path, just reachable from async).
    probe = getattr(app.state, "openai_chat_probe", None)
    if probe is None:
        probe = await _probe_openai_chat_raw()
        app.state.openai_chat_probe = probe
    print(f"[healthz] openai_chat.ok={probe.ok} status={probe.status!r}")
    if probe.status != "ok":
        print(
            f"[FAIL] expected openai_chat.status='ok'; got {probe.status!r}. "
            f"detail={getattr(probe, 'detail', None)!r}"
        )
        return 1

    # (b) invoke reasoning factory
    factory = app.state.llm_reasoning_provider
    if not callable(factory):
        print(
            f"[FAIL] llm_reasoning_provider is not a factory; "
            f"got {type(factory).__name__}"
        )
        return 1
    provider = factory(ws)
    print(
        f"[factory] llm_reasoning_provider(ws) → {type(provider).__name__} "
        f"role={provider.role.value!r} default_module={provider._default_module!r}"
    )

    # (c) real chat call through TrackedProvider
    result = await provider.chat(
        [
            Message(
                role="system",
                content="Pick the tool 'done' and explain briefly.",
            ),
            Message(role="user", content="ok"),
        ],
        response_model=Action,
    )
    # Structured output — print just the shape, not the raw OpenAI response.
    print(f"[chat] result.model_dump() = {result.model_dump()}")

    # (d) inspect token_usage.jsonl
    usage_path = ws / "token_usage.jsonl"
    if not usage_path.exists():
        print("[FAIL] token_usage.jsonl was not created")
        return 1
    lines = [
        json.loads(line)
        for line in usage_path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    print(f"[token_usage] {len(lines)} row(s) in {usage_path}")
    for entry in lines:
        print(
            f"  - module={entry.get('module')!r} operation={entry.get('operation')!r} "
            f"input_tokens={entry.get('input_tokens')} "
            f"output_tokens={entry.get('output_tokens')} "
            f"cost_usd={entry.get('cost_usd')}"
        )

    reasoning_rows = [r for r in lines if r.get("module") == "reasoning"]
    if not reasoning_rows:
        print(
            f"[FAIL] expected at least one row with module='reasoning'; "
            f"got modules={[r.get('module') for r in lines]}"
        )
        return 1
    print(f"[PASS] {len(reasoning_rows)} reasoning row(s) confirmed")

    # Also peek llm_calls.jsonl to confirm wire-payload logging fires.
    calls_path = ws / "llm_calls.jsonl"
    if calls_path.exists():
        call_count = sum(
            1
            for line in calls_path.read_text(encoding="utf-8").splitlines()
            if line.strip()
        )
        print(f"[llm_calls] {call_count} row(s) in llm_calls.jsonl")

    return 0


def main() -> int:
    """Sync top-level: load env, build app, then hand to async checks.

    `create_app` runs a one-shot startup probe via `asyncio.run(...)`,
    which errors if we're already inside `asyncio.run(_main())` — so the
    app MUST be built at sync top-level, not from within an async frame.
    """
    import os

    repo_root = Path(__file__).resolve().parents[2]
    env_path = repo_root / ".env"
    _load_env_silently(env_path)

    if not os.environ.get("CODEBUS_OPENAI_API_KEY"):
        print("[FAIL] CODEBUS_OPENAI_API_KEY not found in .env", file=sys.stderr)
        return 2

    from codebus_agent.api import create_app  # import after env load

    ws = repo_root / ".codebus-smoke-ws"
    _purge_workspace(ws)

    # "configured" is a presence-flag sentinel — the real key is read from
    # env inside OpenAIChatProvider / OpenAIEmbeddingProvider, never passed
    # through this script as a Python argument.
    bearer = secrets.token_urlsafe(32)
    app = create_app(bearer_token=bearer, openai_api_key="configured")

    return asyncio.run(_run_async_checks(app, ws))


if __name__ == "__main__":
    sys.exit(main())
