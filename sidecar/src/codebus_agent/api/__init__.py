"""FastAPI application factory.

Backs:
- openspec/changes/m1-power-on/specs/sidecar-runtime/spec.md
    Requirements: Bearer token authentication, Health endpoint
- openspec/changes/qdrant-lifecycle-bootstrap/specs/qdrant-client/spec.md
    Requirements:
      - Async Qdrant client lifecycle bound to FastAPI app
      - Runtime health endpoint reflects Qdrant connectivity
- openspec/changes/kb-build-production-wiring/specs/sidecar-runtime/spec.md
    Requirement: KB dependency injection hook

Qdrant is wired in as a first-class runtime dependency: if the caller
passes ``qdrant_url``, the factory constructs a single ``AsyncQdrantClient``
on ``app.state.qdrant_client`` (per design「single async client，app state
常駐」) and auto-registers a probe-backed dependency check so ``/healthz``
mirrors live connectivity. Construction never touches the network, so
a missing Qdrant does not block startup (design「degraded-but-alive」).

KB build production wiring (D-032):
  - ``openai_api_key`` kwarg threads through to ``wire_kb_dependencies``.
  - Factory pattern for workspace-scoped slots (``kb_provider`` /
    ``kb_usage_tracker``) so audit logs land at
    ``<workspace>/token_usage.jsonl`` etc. even though the sidecar does
    not know the workspace at startup.
  - Missing ``openai_api_key`` leaves KB slots ``None`` — ``POST /kb/build``
    responds ``503 KB_NOT_CONFIGURED`` downstream; sidecar stays alive.
  - ``/healthz`` gets an ``openai_embedding`` dependency probe reflecting
    one of ``ok`` / ``degraded`` / ``not-configured``. The startup smoke
    probe uses a RAW ``OpenAIEmbeddingProvider`` (not TrackedProvider) —
    operational check MUST NOT pollute workspace audit trail.
"""
from __future__ import annotations

import asyncio
import logging
from pathlib import Path
from typing import Any, Callable

from fastapi import FastAPI

from codebus_agent import auth
from codebus_agent.api.kb import router as kb_router
from codebus_agent.api.scan import router as scan_router
from codebus_agent.api.tasks import TaskRegistry, router as tasks_router
from codebus_agent.health import DependencyCheck, DependencyStatus, collect
from codebus_agent.kb import qdrant_client as _kb_qdrant
from codebus_agent.kb.backend import QdrantHttpBackend
from codebus_agent.providers import (
    OPENAI_EMBEDDING_DIM,
    LLMCallLogger,
    OpenAIChatProvider,
    OpenAIEmbeddingProvider,
    ProviderRole,
    TrackedProvider,
    UsageTracker,
)
from codebus_agent.providers.protocol import Message
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine
from pydantic import BaseModel

logger = logging.getLogger(__name__)


_WORKSPACE_AUDIT_SUBDIR = ".codebus"
_SANITIZE_AUDIT_FILENAME = "sanitize_audit.jsonl"

# Kept in sync with `sidecar/src/codebus_agent/sanitizer/config.py::_BUILTIN_RULES_VERSION`
# and `api/scan.py::_RULES_VERSION`. Bumping one SHALL bump all three
# (docs/sanitizer.md §六 / CLAUDE.md invariant #9).
_RULES_VERSION = "2026-04-20-1"


def wire_kb_dependencies(
    app: FastAPI,
    *,
    openai_api_key: str | None,
    qdrant_url: str | None,
) -> None:
    """Populate the four ``app.state.kb_*`` slots.

    Backs openspec/changes/kb-build-production-wiring/specs/sidecar-runtime/spec.md
      Requirement: KB dependency injection hook

    Contract summary:
      * ``kb_backend``  — ``QdrantHttpBackend`` instance (app-level) when
        ``qdrant_url`` is set; else ``None``.
      * ``kb_provider`` — ``Callable[[Path], TrackedProvider]`` factory
        when ``openai_api_key`` is set; else ``None``. Factory returns a
        ``TrackedProvider`` whose ``UsageTracker`` / ``LLMCallLogger`` /
        ``SanitizerAuditLogger`` resolve under the given workspace root.
      * ``kb_usage_tracker`` — ``Callable[[Path], UsageTracker]`` factory
        when ``openai_api_key`` is set; else ``None``.
      * ``kb_embedding_dim`` — ``OPENAI_EMBEDDING_DIM`` constant when
        ``openai_api_key`` is set; else ``None``.

    Asymmetry (``kb_backend`` / ``kb_embedding_dim`` are not factories):
    the Qdrant client is a shared connection and the embedding dim is
    the constant 1536 (D-032 decision 1). Only audit components need
    workspace scoping.
    """
    if qdrant_url is not None:
        app.state.kb_backend = QdrantHttpBackend(app.state.qdrant_client)
    else:
        app.state.kb_backend = None

    if openai_api_key:
        app.state.kb_provider = _make_provider_factory(default_module="kb_build")
        # `kb-query-endpoint`: distinct factory for the query path so
        # `token_usage.jsonl` lines from `/kb/query` are tagged
        # `module="kb_query"` (vs `"kb_build"` from `/kb/build`),
        # letting cost accounting split build vs query without per-call
        # `module=` plumbing in the endpoint handlers.
        app.state.kb_query_provider = _make_provider_factory(
            default_module="kb_query"
        )
        app.state.kb_usage_tracker = _make_tracker_factory()
        app.state.kb_embedding_dim = OPENAI_EMBEDDING_DIM
        # `chat-provider-wiring`: three chat-ish role factories share the
        # same OpenAI key + `gpt-4o-mini` default model, differing only in
        # temperature (reasoning: 0.1 deterministic, judge: 0.0 strictly
        # deterministic, chat: 0.2 slightly creative) and `default_module`
        # tag so `token_usage.jsonl` can split cost by role without any
        # per-call plumbing. All three MUST be wrapped in `TrackedProvider`
        # (registry guard + ALLOWED_INNER_TYPES allowlist enforce this).
        app.state.llm_reasoning_provider = _make_chat_provider_factory(
            model="gpt-4o-mini",
            temperature=0.1,
            default_module="reasoning",
            role=ProviderRole.REASONING,
        )
        app.state.llm_judge_provider = _make_chat_provider_factory(
            model="gpt-4o-mini",
            temperature=0.0,
            default_module="judge",
            role=ProviderRole.JUDGE,
        )
        app.state.llm_chat_provider = _make_chat_provider_factory(
            model="gpt-4o-mini",
            temperature=0.2,
            default_module="chat",
            role=ProviderRole.CHAT,
        )
    else:
        app.state.kb_provider = None
        app.state.kb_query_provider = None
        app.state.kb_usage_tracker = None
        app.state.kb_embedding_dim = None
        app.state.llm_reasoning_provider = None
        app.state.llm_judge_provider = None
        app.state.llm_chat_provider = None


def _make_tracker_factory() -> Callable[[Path], UsageTracker]:
    """Factory for workspace-scoped UsageTracker (D-021 workspace-level path)."""

    def _factory(workspace_root: Path) -> UsageTracker:
        return UsageTracker(Path(workspace_root) / "token_usage.jsonl")

    return _factory


def _make_provider_factory(
    *, default_module: str
) -> Callable[[Path], TrackedProvider]:
    """Factory for workspace-scoped TrackedProvider wrapping OpenAIEmbeddingProvider.

    TrackedProvider binds three audit loggers at construction time, all
    workspace-scoped:
      * ``UsageTracker`` → ``<ws>/token_usage.jsonl`` (D-021)
      * ``LLMCallLogger`` → ``<ws>/llm_calls.jsonl`` (D-022)
      * ``SanitizerAuditLogger`` → ``<ws>/.codebus/sanitize_audit.jsonl``

    Constructing the raw ``OpenAIEmbeddingProvider`` inside the factory
    (vs. once at startup) is acceptable: the openai SDK is inexpensive
    to instantiate relative to a multi-minute KB build.

    ``default_module`` is parameterized (``kb-query-endpoint``) so the
    same factory shape can produce ``kb_build``-tagged providers for the
    build path and ``kb_query``-tagged providers for the query path,
    splitting cost in ``token_usage.jsonl`` without per-call plumbing.
    """

    def _factory(workspace_root: Path) -> TrackedProvider:
        ws = Path(workspace_root)
        tracker = UsageTracker(ws / "token_usage.jsonl")
        call_logger = LLMCallLogger(ws / "llm_calls.jsonl")
        sanitizer_audit_path = ws / _WORKSPACE_AUDIT_SUBDIR / _SANITIZE_AUDIT_FILENAME
        sanitizer_audit = SanitizerAuditLogger(sanitizer_audit_path)
        return TrackedProvider(
            OpenAIEmbeddingProvider(),
            tracker=tracker,
            logger=call_logger,
            role=ProviderRole.EMBED,
            sanitizer=SanitizerEngine(),
            sanitizer_audit=sanitizer_audit,
            rules_version=_RULES_VERSION,
            # `usage-tracker-dedup`: TrackedProvider tags every record
            # with this module label so `token_usage.jsonl` lines are
            # aggregable / billable to the right subsystem without the
            # caller needing to plumb `module=` through every call.
            default_module=default_module,
        )

    return _factory


def _make_chat_provider_factory(
    *,
    model: str,
    temperature: float,
    default_module: str,
    role: ProviderRole,
) -> Callable[[Path], TrackedProvider]:
    """Factory for workspace-scoped TrackedProvider wrapping OpenAIChatProvider.

    Mirrors `_make_provider_factory` but for chat-ish roles: the inner
    provider is `OpenAIChatProvider` (not `OpenAIEmbeddingProvider`), the
    TrackedProvider role is caller-supplied (`REASONING` / `JUDGE` /
    `CHAT`) rather than hard-wired to `EMBED`, and temperature is
    parameterized so callers can tune per role without spinning up a new
    factory type per variant.

    `default_module` still flows straight into `TrackedProvider` so every
    `token_usage.jsonl` line written via this factory's returned wrapper
    gets tagged `module=<reasoning|judge|chat>`.
    """

    def _factory(workspace_root: Path) -> TrackedProvider:
        ws = Path(workspace_root)
        tracker = UsageTracker(ws / "token_usage.jsonl")
        call_logger = LLMCallLogger(ws / "llm_calls.jsonl")
        sanitizer_audit_path = ws / _WORKSPACE_AUDIT_SUBDIR / _SANITIZE_AUDIT_FILENAME
        sanitizer_audit = SanitizerAuditLogger(sanitizer_audit_path)
        return TrackedProvider(
            OpenAIChatProvider(model, temperature=temperature),
            tracker=tracker,
            logger=call_logger,
            role=role,
            sanitizer=SanitizerEngine(),
            sanitizer_audit=sanitizer_audit,
            rules_version=_RULES_VERSION,
            default_module=default_module,
        )

    return _factory


class _ChatProbeModel(BaseModel):
    """Minimal Pydantic shape for the chat smoke probe.

    Kept tiny on purpose: the probe only proves the OpenAI chat endpoint
    is reachable + Instructor TOOLS-mode round-trip works; we don't need
    a real payload.
    """

    ok: bool = True


async def _probe_openai_chat_raw() -> DependencyStatus:
    """Smoke-check OpenAI chat completions with a raw (non-tracked) provider.

    Spec scenario `Healthz reflects OpenAI chat configuration state`:
    one probe covers all three chat-ish roles since they share the same
    OpenAI API + key. Like the embedding probe, this bypass is permitted
    because an operational health check is not production traffic and
    MUST NOT write to any workspace audit trail (`token_usage.jsonl` /
    `llm_calls.jsonl` / `sanitize_audit.jsonl`).
    """
    try:
        provider = OpenAIChatProvider("gpt-4o-mini")
        await provider.chat(
            [Message(role="user", content="ping")],
            response_model=_ChatProbeModel,
        )
    except Exception as exc:  # noqa: BLE001 — classify broadly, details in detail
        return DependencyStatus(
            ok=False,
            status="degraded",
            detail=f"{type(exc).__name__}",
        )
    return DependencyStatus(ok=True, status="ok")


async def _probe_openai_chat_not_configured() -> DependencyStatus:
    """Probe returned when `CODEBUS_OPENAI_API_KEY` is absent.

    Mirrors `_probe_openai_embedding_not_configured`: `ok=True` because
    this is an *expected* degraded state, not a failure.
    """
    return DependencyStatus(
        ok=True,
        status="not-configured",
        detail="CODEBUS_OPENAI_API_KEY not set",
    )


async def _probe_openai_embedding_raw() -> DependencyStatus:
    """Smoke-check OpenAI embeddings with a raw (non-tracked) provider.

    Spec scenario ``Healthz smoke probe bypasses TrackedProvider``:
    this probe intentionally does NOT go through ``TrackedProvider``
    because an operational health check is not production traffic and
    MUST NOT write to any workspace audit trail.
    """
    try:
        provider = OpenAIEmbeddingProvider()
        await provider.embed(["ping"])
    except Exception as exc:  # noqa: BLE001 — classify broadly, details in detail
        return DependencyStatus(
            ok=False,
            status="degraded",
            detail=f"{type(exc).__name__}",
        )
    return DependencyStatus(ok=True, status="ok")


async def _probe_openai_embedding_not_configured() -> DependencyStatus:
    """Probe returned when ``CODEBUS_OPENAI_API_KEY`` is absent.

    ``ok=True`` because this is an *expected* degraded state — the
    sidecar is intentionally running without KB build capability, not
    failing. Callers distinguish the case via ``status == "not-configured"``.
    """
    return DependencyStatus(
        ok=True,
        status="not-configured",
        detail="CODEBUS_OPENAI_API_KEY not set",
    )


def create_app(
    bearer_token: str,
    dependency_checks: dict[str, DependencyCheck] | None = None,
    qdrant_url: str | None = None,
    openai_api_key: str | None = None,
) -> FastAPI:
    """Build the sidecar FastAPI application.

    The bearer token is passed in at construction time so it lives only
    in memory for the lifetime of this process, per D-local-2.

    ``dependency_checks`` is injected so tests (and M2+ wiring) can plug
    in custom probes. When ``qdrant_url`` is given, a Qdrant probe is
    auto-bound under the ``"qdrant"`` key unless the caller overrides it.

    ``openai_api_key`` threads through to ``wire_kb_dependencies`` per
    the ``kb-build-production-wiring`` change. When ``None``, KB slots
    stay ``None`` and ``POST /kb/build`` returns ``503 KB_NOT_CONFIGURED``.
    """
    if not bearer_token or len(bearer_token) < 32:
        raise ValueError("bearer_token must be at least 32 characters")
    app = FastAPI(title="codebus-sidecar", version="0.1.0")
    app.state.bearer_token = bearer_token
    app.state.qdrant_client = None
    # Single-slot task registry — survives the lifetime of the app, holds at
    # most one in-flight background task per spec
    # `sse-progress-skeleton/sidecar-runtime` Requirement
    # `Single-slot in-memory task registry`.
    app.state.tasks = TaskRegistry()

    checks: dict[str, DependencyCheck] = dict(dependency_checks or {})

    if qdrant_url is not None:
        app.state.qdrant_client = _kb_qdrant.build_client(qdrant_url)

        if "qdrant" not in checks:
            async def _qdrant_probe() -> DependencyStatus:
                return await asyncio.to_thread(_kb_qdrant.probe, qdrant_url)

            checks["qdrant"] = _qdrant_probe

        @app.on_event("shutdown")
        async def _close_qdrant() -> None:
            client = getattr(app.state, "qdrant_client", None)
            if client is not None:
                await client.close()

    # Wire KB deps BEFORE the healthz probe, so the probe can dispatch
    # against freshly-populated slots without re-reading env vars.
    wire_kb_dependencies(app, openai_api_key=openai_api_key, qdrant_url=qdrant_url)

    # Run the startup smoke probes exactly once; cache their results so
    # /healthz doesn't hit OpenAI on every request. The cached statuses
    # live on app.state so tests can introspect them if needed.
    if openai_api_key:
        try:
            _embed_probe = asyncio.run(_probe_openai_embedding_raw())
        except RuntimeError:
            # asyncio.run fails if there's a running loop (TestClient can
            # trigger this on some paths). Fall back to the probe being
            # evaluated lazily.
            _embed_probe = None
        app.state.openai_embedding_probe = _embed_probe

        async def _cached_openai_embedding_probe() -> DependencyStatus:
            cached = getattr(app.state, "openai_embedding_probe", None)
            if cached is not None:
                return cached
            fresh = await _probe_openai_embedding_raw()
            app.state.openai_embedding_probe = fresh
            return fresh

        checks["openai_embedding"] = _cached_openai_embedding_probe

        # `chat-provider-wiring`: parallel smoke probe for the chat path.
        # One probe covers all three chat-ish roles (REASONING / JUDGE /
        # CHAT) since they share the same OpenAI API + key.
        try:
            _chat_probe = asyncio.run(_probe_openai_chat_raw())
        except RuntimeError:
            _chat_probe = None
        app.state.openai_chat_probe = _chat_probe

        async def _cached_openai_chat_probe() -> DependencyStatus:
            cached = getattr(app.state, "openai_chat_probe", None)
            if cached is not None:
                return cached
            fresh = await _probe_openai_chat_raw()
            app.state.openai_chat_probe = fresh
            return fresh

        checks["openai_chat"] = _cached_openai_chat_probe
    else:
        checks["openai_embedding"] = _probe_openai_embedding_not_configured
        checks["openai_chat"] = _probe_openai_chat_not_configured

    app.state.dependency_checks = checks
    auth.install(app, bearer_token)

    @app.get("/healthz")
    async def healthz() -> dict[str, object]:
        report = await collect(app.state.dependency_checks)
        return report.to_dict()

    # Scanner router — 註冊於 bearer middleware 下（install 先於 include_router）
    # 對齊 spec「Workspace scan endpoint」：endpoint MUST NOT bypass bearer middleware。
    app.include_router(scan_router)
    # Task lifecycle (`GET /tasks/{id}/events|result`) + KB build (`POST /kb/build`)
    # 都掛在同一層 bearer middleware 下；spec
    # `sse-progress-skeleton/sidecar-runtime` 規定 SSE 與 result endpoint MUST
    # NOT bypass bearer。
    app.include_router(tasks_router)
    app.include_router(kb_router)

    return app


__all__: list[str] = ["create_app", "wire_kb_dependencies"]
