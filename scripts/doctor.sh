#!/usr/bin/env bash
# `make doctor` — is this machine ready to build and gate hippo-task?
# Fast and read-only. Exit 1 if anything required is
# missing; optional tools only WARN.
set -euo pipefail
cd "$(dirname "$0")/.."

fail=0
ok()   { echo "  ok   $*"; }
miss() { echo "  MISS $*"; fail=1; }
warn() { echo "  WARN $*"; }

# a >= b for dotted versions, without relying on `sort -V`.
version_ge() {
  awk -v a="$1" -v b="$2" 'BEGIN {
    split(a, x, "."); split(b, y, ".")
    for (i = 1; i <= 3; i++) { if (x[i] + 0 > y[i] + 0) exit 0; if (x[i] + 0 < y[i] + 0) exit 1 }
    exit 0 }'
}

echo "doctor: toolchain"
if command -v cargo >/dev/null 2>&1; then
  ok "cargo: $(cargo --version)"
else
  miss "cargo — install Rust from https://rustup.rs"
fi
msrv="$(sed -n 's/^rust-version *= *"\(.*\)"/\1/p' Cargo.toml)"
if command -v rustc >/dev/null 2>&1; then
  have="$(rustc --version | awk '{print $2}')"
  if version_ge "$have" "$msrv"; then ok "rustc $have (needs ≥ $msrv)"; else miss "rustc $have is older than $msrv — run: rustup update"; fi
else
  miss "rustc"
fi
if cargo fmt --version >/dev/null 2>&1; then ok "rustfmt"; else miss "rustfmt — run: make setup"; fi
if cargo clippy --version >/dev/null 2>&1; then ok "clippy"; else miss "clippy — run: make setup"; fi
if command -v python3 >/dev/null 2>&1; then ok "python3: $(python3 --version)"; else warn "python3 — not needed for make verify, but \`make ui\` (the playground UI) needs it"; fi

echo "doctor: repo"
for f in Cargo.lock README.md AGENTS.md CHANGELOG.md scripts/check-test-integrity.sh; do
  if [ -f "$f" ]; then ok "$f"; else miss "$f"; fi
done
# Stale self-reference check: the version being built has a CHANGELOG entry
# (tests/docs.rs enforces this too, so CI catches it).
version="$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -1)"
if grep -q "^## \[$version\]" CHANGELOG.md 2>/dev/null; then
  ok "CHANGELOG has an entry for $version"
else
  miss "CHANGELOG.md has no '## [$version]' entry"
fi

if [ "$fail" -ne 0 ]; then echo "doctor: FAIL"; exit 1; fi
echo "doctor: PASS"
