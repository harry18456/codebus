"""Live repro of the demo-synthetic KB build INTERNAL_ERROR.

This is NOT a unit test — it exercises the full live OpenAI + Qdrant
path and is gated on env vars, so it skips by default in CI / clean
checkouts and runs only when:
  * `CODEBUS_OPENAI_API_KEY` is set in env
  * Qdrant on 127.0.0.1:6333 is reachable

Used to capture the actual Python traceback that ends up classified
INTERNAL_ERROR in the wire SSE error event when the user runs
`cargo tauri dev` against `tests/golden/demo-synthetic`.

Run manually:
    cd sidecar
    uv run pytest tests/test_kb_build_repro.py -s --no-header
"""
from __future__ import annotations

import os
import urllib.request
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
DEMO_WS = REPO_ROOT / "tests" / "golden" / "demo-synthetic"


def _qdrant_reachable() -> bool:
    try:
        with urllib.request.urlopen(
            "http://127.0.0.1:6333/healthz", timeout=0.5
        ) as resp:
            return 200 <= resp.status < 300
    except Exception:
        return False


def _load_env() -> None:
    env_path = REPO_ROOT / ".env"
    if not env_path.exists():
        return
    for raw in env_path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        k, _, v = line.partition("=")
        k = k.strip()
        v = v.strip().strip('"').strip("'")
        if k and not os.environ.get(k):
            os.environ[k] = v


@pytest.mark.skipif(
    not DEMO_WS.is_dir(), reason=f"demo workspace missing: {DEMO_WS}"
)
@pytest.mark.asyncio
async def test_demo_synthetic_kb_build_live() -> None:
    _load_env()
    if not os.environ.get("CODEBUS_OPENAI_API_KEY"):
        pytest.skip("CODEBUS_OPENAI_API_KEY not in env / .env")
    if not _qdrant_reachable():
        pytest.skip("Qdrant not reachable on 127.0.0.1:6333")

    from codebus_agent.kb import qdrant_client as _kb_qdrant
    from codebus_agent.kb.backend import QdrantHttpBackend
    from codebus_agent.kb.knowledge_base import KnowledgeBase
    from codebus_agent.providers import (
        LLMCallLogger,
        OpenAIEmbeddingProvider,
        ProviderRole,
        TrackedProvider,
        UsageTracker,
    )
    from codebus_agent.sanitizer import (
        RULES_VERSION,
        SanitizerAuditLogger,
        SanitizerEngine,
    )
    from codebus_agent.sandbox import ToolContext
    from codebus_agent.scanner.service import scan

    qdrant_url = _kb_qdrant.resolve_url()
    qdrant_client = _kb_qdrant.build_client(qdrant_url)
    backend = QdrantHttpBackend(qdrant_client)

    audit_dir = DEMO_WS / ".codebus"
    audit_dir.mkdir(parents=True, exist_ok=True)

    tracker = UsageTracker(audit_dir / "token_usage.jsonl")
    call_logger = LLMCallLogger(audit_dir / "llm_calls.jsonl")
    sanitizer_audit = SanitizerAuditLogger(audit_dir / "sanitize_audit.jsonl")
    provider = TrackedProvider(
        OpenAIEmbeddingProvider(),
        tracker=tracker,
        logger=call_logger,
        role=ProviderRole.EMBED,
        sanitizer=SanitizerEngine(),
        sanitizer_audit=sanitizer_audit,
        rules_version=RULES_VERSION,
        default_module="kb_build",
    )

    print(f"\n[repro] scanning {DEMO_WS}", flush=True)
    ctx = ToolContext(
        workspace_root=DEMO_WS,
        workspace_type="folder",
        sanitizer=SanitizerEngine(),
    )
    scan_result = await scan(str(DEMO_WS), ctx, sanitize_audit=sanitizer_audit)
    print(f"[repro] scan complete: {len(scan_result.files)} files", flush=True)

    kb = KnowledgeBase(
        backend=backend,
        provider=provider,
        usage_tracker=tracker,
        workspace_root=str(DEMO_WS),
        embedding_dim=1536,
    )

    progress_events: list[dict] = []

    async def _on_progress(event) -> None:
        progress_events.append(event)
        phase = getattr(event, "phase", "?")
        cur = getattr(event, "current", "?")
        total = getattr(event, "total", "?")
        print(f"[repro] progress: {phase} {cur}/{total}", flush=True)

    try:
        stats = await kb.build(scan_result, on_progress=_on_progress)
    except BaseException as exc:
        import traceback as _tb

        print("\n[repro] !!! KB build raised !!!", flush=True)
        print(f"[repro] type: {type(exc).__name__}", flush=True)
        print(f"[repro] message: {exc}", flush=True)
        print(f"[repro] last 3 progress events: {progress_events[-3:]}", flush=True)
        _tb.print_exc()
        raise

    print(f"\n[repro] PASS: {stats}", flush=True)
