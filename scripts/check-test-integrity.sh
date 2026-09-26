#!/usr/bin/env bash
# Deterministic test-integrity gate.
#
# Tests are this project's main guardrail, so they may be *strengthened* freely
# but not silently *weakened*. Compared with a base revision, this fails on:
#   - removed assertions   (assert!, assert_eq!, assert_ne!, debug_assert…)
#   - newly ignored tests  (#[ignore])
# Override for a deliberate change: put "test-weaken-ok: <reason>" in the diff
# and get it reviewed.
#
# Base revision: $INTEGRITY_BASE if set (CI passes the PR base / previous
# commit), else HEAD (i.e. your uncommitted changes). Without git history the
# gate reports that it is skipping and passes.
set -euo pipefail
cd "$(dirname "$0")/.."

if ! git rev-parse --verify HEAD >/dev/null 2>&1; then
  echo "test-integrity: no git history to compare against (skipping)"; exit 0
fi
base="${INTEGRITY_BASE:-HEAD}"
if ! git rev-parse --verify --quiet "$base^{commit}" >/dev/null; then
  echo "test-integrity: base '$base' not found (first push?) — skipping"; exit 0
fi

# Unit tests live next to the code in src/, integration tests in tests/.
diff="$(git diff "$base" -- tests/ src/ 2>/dev/null || true)"
if [ -z "$diff" ]; then
  echo "✅ test-integrity: no test changes (vs $base)"; exit 0
fi

if printf '%s\n' "$diff" | grep -qE '^\+.*test-weaken-ok:'; then
  echo "⚠️  test-integrity: weakening explicitly justified (test-weaken-ok) — allowed"; exit 0
fi

# Ignore comment lines (prose that merely mentions "assert" or "ignore").
strip_comments='^[+-][[:space:]]*//'
removed_assertions="$(printf '%s\n' "$diff" | grep -E '^-[^-].*(assert(_eq|_ne)?!|debug_assert)' | grep -vE "$strip_comments" || true)"
added_ignores="$(printf '%s\n' "$diff" | grep -E '^\+.*#\[ignore' | grep -vE "$strip_comments" || true)"

if [ -n "$removed_assertions" ] || [ -n "$added_ignores" ]; then
  echo "❌ test-integrity: weakening detected (fails verify)"
  if [ -n "$removed_assertions" ]; then
    echo "  removed/loosened assertions:"; printf '%s\n' "$removed_assertions" | sed 's/^/    /'
  fi
  if [ -n "$added_ignores" ]; then
    echo "  newly ignored tests:"; printf '%s\n' "$added_ignores" | sed 's/^/    /'
  fi
  echo "  → Fix the code, not the test. If truly intentional, add 'test-weaken-ok: <reason>' and get it reviewed."
  exit 1
fi

echo "✅ test-integrity: tests changed vs $base, nothing weakened (strengthening is welcome)"
