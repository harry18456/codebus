"""Golden-sample scoring helpers — recall / noise / composite + IdealRoute schema.

Backs SHALL clauses in
openspec/changes/golden-sample-baseline/specs/explorer-golden/spec.md
  Requirement: Golden scoring helpers compute recall, noise, and composite score

Test-only utility per design Decision 1 (D-006). Stays out of the
production package: no `codebus_agent.*` imports — `pydantic` is the
only dependency. Any future fixture (Explorer / Q&A / Generator) can
reuse these helpers to score produced station sets against handwritten
ideal routes.
"""
from __future__ import annotations

from pydantic import BaseModel

__all__ = [
    "IdealRoute",
    "composite_score",
    "station_noise",
    "station_recall",
]


_DEFAULT_WEIGHTS: dict[str, float] = {"recall": 0.5, "noise": 0.3, "depth": 0.2}
_REQUIRED_WEIGHT_KEYS: frozenset[str] = frozenset({"recall", "noise", "depth"})


def station_recall(
    produced_paths: set[str], must_have_paths: set[str]
) -> float:
    """Fraction of `must_have_paths` that the agent actually produced.

    `len(produced_paths & must_have_paths) / len(must_have_paths)`. Empty
    `must_have_paths` raises `ValueError` so callers cannot silently
    divide by zero — design Decision 1 explicitly forbids returning 1.0
    or 0.0 for the empty case (both readings are misleading).
    """
    if not must_have_paths:
        raise ValueError("must_have_paths cannot be empty")
    return len(produced_paths & must_have_paths) / len(must_have_paths)


def station_noise(
    produced_paths: set[str],
    must_have: set[str],
    nice_to_have: set[str],
) -> float:
    """Fraction of off-route extras that are also outside `nice_to_have`.

    `extras = produced_paths - must_have`. When `extras` is empty the
    function returns `0.0` (no extras → no noise; clean output, NOT an
    error) per spec scenario `station_noise returns zero when extras is
    empty`. Otherwise: `len(extras - nice_to_have) / len(extras)`.
    """
    extras = produced_paths - must_have
    if not extras:
        return 0.0
    return len(extras - nice_to_have) / len(extras)


def composite_score(
    recall: float,
    noise: float,
    depth: float,
    weights: dict[str, float] | None = None,
) -> float:
    """D-006 weighted score: `w_r * recall + w_n * (1 - noise) + w_d * depth`.

    Default weights `{"recall": 0.5, "noise": 0.3, "depth": 0.2}`. Passing
    a partial `weights` override raises `KeyError` so silent default
    substitution does not mask tuning intent (spec scenario
    `composite_score requires all three weight keys when overridden`).
    """
    if weights is None:
        w = _DEFAULT_WEIGHTS
    else:
        missing = _REQUIRED_WEIGHT_KEYS - weights.keys()
        if missing:
            raise KeyError(
                f"composite_score weights override missing keys: "
                f"{sorted(missing)!r} (override MUST contain all of "
                f"{sorted(_REQUIRED_WEIGHT_KEYS)!r})"
            )
        w = weights
    return w["recall"] * recall + w["noise"] * (1.0 - noise) + w["depth"] * depth


class IdealRoute(BaseModel):
    """Machine-readable ideal exploration route per fixture.

    The four fields have no defaults so callers MUST populate every
    list explicitly; orphan paths in the workspace are then enforced
    by `Timeline-storage-adapter-synthetic fixture pins ideal-route
    stations` scenario `All workspace files appear in the ideal route
    schema`. Pydantic v2 round-trips bit-identically through
    `model_dump_json` / `model_validate_json` — that contract is what
    `tests/golden/<fixture>/ideal-route.json` files rely on.
    """

    task: str
    must_have: list[str]
    nice_to_have: list[str]
    noise_paths: list[str]
