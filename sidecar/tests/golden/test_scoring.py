"""Unit tests for `sidecar/tests/golden/scoring.py`.

Backs SHALL clauses in
openspec/changes/golden-sample-baseline/specs/explorer-golden/spec.md
  Requirement: Golden scoring helpers compute recall, noise, and composite score

Eight spec scenarios driven directly:
  - station_recall: perfect / no / partial / empty must_have raises ValueError
  - station_noise: pure / nice_to_have-as-not-noise / real-noise / empty extras
  - composite_score: default D-006 weights / override missing keys raises KeyError
  - IdealRoute round-trips through model_dump_json / model_validate_json
"""
from __future__ import annotations

import pytest

from .scoring import IdealRoute, composite_score, station_noise, station_recall


# ----------------------------- station_recall -----------------------------


def test_station_recall_returns_one_on_perfect_hit() -> None:
    assert station_recall({"a", "b", "c"}, {"a", "b", "c"}) == 1.0


def test_station_recall_returns_zero_on_no_hit() -> None:
    assert station_recall({"x", "y"}, {"a", "b", "c"}) == 0.0


def test_station_recall_returns_partial_fraction_on_partial_hit() -> None:
    assert station_recall({"a", "x"}, {"a", "b", "c"}) == pytest.approx(1.0 / 3.0)


def test_station_recall_raises_on_empty_must_have() -> None:
    with pytest.raises(ValueError, match="must_have_paths cannot be empty"):
        station_recall(set(), set())


# ------------------------------ station_noise -----------------------------


def test_station_noise_pure_hits_no_noise() -> None:
    assert station_noise({"a"}, {"a"}, set()) == 0.0


def test_station_noise_treats_nice_to_have_as_not_noise() -> None:
    # extras = {"n"}, all in nice_to_have → noise = 0/1 = 0.0
    assert station_noise({"a", "n"}, {"a"}, {"n"}) == 0.0


def test_station_noise_returns_half_on_real_noise() -> None:
    # extras = {"n", "x"}, real_noise = {"x"} → 1/2 = 0.5
    assert station_noise({"a", "n", "x"}, {"a"}, {"n"}) == 0.5


def test_station_noise_returns_zero_when_extras_empty() -> None:
    # produced is a subset of must_have → extras = ∅ → 0.0 (NOT raise)
    assert station_noise({"a"}, {"a"}, set()) == 0.0


# ----------------------------- composite_score ----------------------------


def test_composite_score_default_weights_match_d006_formula() -> None:
    # 0.5 * 1.0 + 0.3 * (1 - 0.0) + 0.2 * 1.0 == 1.0
    assert composite_score(1.0, 0.0, 1.0) == pytest.approx(1.0)


def test_composite_score_requires_all_three_weight_keys_when_overridden() -> None:
    with pytest.raises(KeyError):
        composite_score(0.8, 0.2, 0.5, weights={"recall": 0.6})


# ------------------------------- IdealRoute -------------------------------


def test_ideal_route_round_trips_through_json() -> None:
    original = IdealRoute(
        task="t",
        must_have=["a"],
        nice_to_have=["b"],
        noise_paths=["c"],
    )
    reloaded = IdealRoute.model_validate_json(original.model_dump_json())
    assert reloaded == original
    assert reloaded.task == "t"
    assert reloaded.must_have == ["a"]
    assert reloaded.nice_to_have == ["b"]
    assert reloaded.noise_paths == ["c"]
