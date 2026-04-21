"""Sample Python module used by chunker tests.

The body is intentionally padded so its tiktoken-measured length
exceeds the default chunk_size of 600, exercising the multi-window
path (line-boundary backtrack + overlap continuity).
"""
from __future__ import annotations


def fibonacci(n: int) -> int:
    """Return the n-th Fibonacci number (iterative).

    The implementation here is deliberately verbose with comments so
    that each function contributes meaningful tokens to the chunker
    tests; do not optimise it.
    """
    if n < 0:
        raise ValueError("n must be non-negative")
    if n == 0:
        return 0
    if n == 1:
        return 1
    previous, current = 0, 1
    for _ in range(2, n + 1):
        previous, current = current, previous + current
    return current


def is_prime(n: int) -> bool:
    """Return True when n is a prime number."""
    if n < 2:
        return False
    if n < 4:
        return True
    if n % 2 == 0:
        return False
    candidate = 3
    while candidate * candidate <= n:
        if n % candidate == 0:
            return False
        candidate += 2
    return True


def factorial(n: int) -> int:
    """Iterative factorial."""
    if n < 0:
        raise ValueError("n must be non-negative")
    result = 1
    for value in range(2, n + 1):
        result *= value
    return result


def sum_of_digits(n: int) -> int:
    """Sum the decimal digits of an integer."""
    if n < 0:
        n = -n
    total = 0
    while n > 0:
        total += n % 10
        n //= 10
    return total


def quicksort(values: list[int]) -> list[int]:
    """Return a new list containing values sorted via quicksort."""
    if len(values) <= 1:
        return list(values)
    pivot = values[len(values) // 2]
    left = [v for v in values if v < pivot]
    middle = [v for v in values if v == pivot]
    right = [v for v in values if v > pivot]
    return quicksort(left) + middle + quicksort(right)


def merge_sort(values: list[int]) -> list[int]:
    """Return a new list containing values sorted via merge sort."""
    if len(values) <= 1:
        return list(values)
    mid = len(values) // 2
    left = merge_sort(values[:mid])
    right = merge_sort(values[mid:])
    merged: list[int] = []
    i = j = 0
    while i < len(left) and j < len(right):
        if left[i] <= right[j]:
            merged.append(left[i])
            i += 1
        else:
            merged.append(right[j])
            j += 1
    merged.extend(left[i:])
    merged.extend(right[j:])
    return merged


def binary_search(values: list[int], target: int) -> int:
    """Return index of target in sorted values, or -1 when absent."""
    low, high = 0, len(values) - 1
    while low <= high:
        mid = (low + high) // 2
        if values[mid] == target:
            return mid
        if values[mid] < target:
            low = mid + 1
        else:
            high = mid - 1
    return -1


def gcd(a: int, b: int) -> int:
    """Greatest common divisor (Euclidean algorithm)."""
    while b:
        a, b = b, a % b
    return abs(a)


def lcm(a: int, b: int) -> int:
    """Least common multiple."""
    if a == 0 or b == 0:
        return 0
    return abs(a * b) // gcd(a, b)
