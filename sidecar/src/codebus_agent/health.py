"""Health / readiness checks.

Backs openspec/changes/m1-power-on/specs/sidecar-runtime/spec.md
  Requirement: Health endpoint

Dependency checks are injected at app-construction time so tests can
swap in in-memory fakes without standing up real Qdrant / other services.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Awaitable, Callable


@dataclass(frozen=True)
class DependencyStatus:
    """Outcome of a single dependency probe.

    ``detail`` is a short human-readable hint (connection refused,
    timeout, bad response, etc.) — surfaced to the parent process so
    the UI can say something more actionable than "degraded".

    ``status`` is an explicit string label that backs the
    ``kb-build-production-wiring`` spec ``KB dependency injection hook``
    scenarios (which require three states: ``ok`` / ``degraded`` /
    ``not-configured``). When ``status`` is unset it is derived from
    ``ok`` so existing call sites keep working unchanged.
    """

    ok: bool
    detail: str = ""
    status: str | None = None

    def to_dict(self) -> dict[str, object]:
        out: dict[str, object] = {
            "ok": self.ok,
            "status": self.status or ("ok" if self.ok else "degraded"),
        }
        if self.detail:
            out["detail"] = self.detail
        return out


DependencyCheck = Callable[[], Awaitable[DependencyStatus]]


@dataclass(frozen=True)
class HealthReport:
    status: str  # "ok" | "degraded"
    dependencies: dict[str, DependencyStatus] = field(default_factory=dict)

    def to_dict(self) -> dict[str, object]:
        return {
            "status": self.status,
            "dependencies": {name: s.to_dict() for name, s in self.dependencies.items()},
        }


async def collect(checks: dict[str, DependencyCheck]) -> HealthReport:
    """Run every check and summarise.

    Status is ``ok`` if (and only if) every dependency reports ok, or
    if there are no checks registered (the M1 default).
    """
    results: dict[str, DependencyStatus] = {}
    for name, check in checks.items():
        try:
            results[name] = await check()
        except Exception as exc:
            results[name] = DependencyStatus(ok=False, detail=f"{type(exc).__name__}: {exc}")
    all_ok = all(s.ok for s in results.values())
    return HealthReport(status="ok" if all_ok else "degraded", dependencies=results)
