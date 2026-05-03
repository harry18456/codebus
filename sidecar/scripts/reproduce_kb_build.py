"""Manual reproduction script for the demo-synthetic KB build INTERNAL_ERROR.

Mirrors the production /kb/build flow against the same workspace the
user is hitting (tests/golden/demo-synthetic) so we can capture the
actual Python traceback that ends up classified INTERNAL_ERROR by
sidecar/api/tasks.py::_classify_exception.

Usage:
    cd sidecar
    uv run python scripts/reproduce_kb_build.py

Loads CODEBUS_OPENAI_API_KEY silently from repo-root .env. Requires
Qdrant running on 127.0.0.1:6333.
"""
from __future__ import annotations

import asyncio
import logging
import os
import sys
import traceback
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DEMO_WS = REPO_ROOT / "tests" / "golden" / "demo-synthetic"


def _load_env_silently() -> None:
    env = REPO_ROOT / ".env"
    if not env.exists():
        raise FileNotFoundError(f".env missing at {env}")
    for raw in env.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        k, _, v = line.partition("=")
        k = k.strip()
        v = v.strip().strip('"').strip("'")
        if k:
            os.environ[k] = v


async def _run() -> None:
    logging.basicConfig(level=logging.INFO, format="[%(levelname)s] %(name)s: %(message)s")

    from codebus_agent.kb import qdrant_client as _kb_qdrant
    from codebus_agent.kb.builder import KnowledgeBase
    from codebus_agent.kb.backend import QdrantHttpBackend
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
    from codebus_agent.scanner.service import scan_workspace

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

    print(f"[repro] scanning {DEMO_WS}")
    scan_result = scan_workspace(workspace_root=str(DEMO_WS))
    print(f"[repro] scan complete: {len(scan_result.files)} files")

    print("[repro] starting KB build")
    kb = KnowledgeBase(
        backend=backend,
        provider=provider,
        usage_tracker=tracker,
        workspace_root=str(DEMO_WS),
        embedding_dim=1536,
    )

    def _on_progress(event: dict) -> None:
        # Print progress events so we can see how far it gets before exception.
        print(f"[repro] progress: {event}")

    try:
        stats = await kb.build(scan_result, on_progress=_on_progress)
    except BaseException as exc:
        print("\n[repro] !!! KB build raised !!!")
        print(f"[repro] type: {type(exc).__name__}")
        print(f"[repro] message: {exc}")
        traceback.print_exc()
        raise

    print(f"\n[repro] PASS: {stats}")


def main() -> int:
    _load_env_silently()
    if not os.environ.get("CODEBUS_OPENAI_API_KEY"):
        print("[repro] CODEBUS_OPENAI_API_KEY missing in .env")
        return 1
    if not DEMO_WS.is_dir():
        print(f"[repro] demo workspace missing: {DEMO_WS}")
        return 1

    try:
        asyncio.run(_run())
        return 0
    except SystemExit:
        raise
    except BaseException:
        return 2


if __name__ == "__main__":
    sys.exit(main())
