#!/bin/sh
# Package one build of hippo-task for download:
#
#   sh scripts/package.sh <target> <path to the built binary> <output folder>
#
# Writes hippo-task-<target>.tar.gz (a .zip for Windows): a folder of the same
# name holding the binary, LICENSE and README.md. Next to it goes
# <archive>.sha256, which the installer checks. The release workflow runs this
# for every target, and tests/install.rs runs it too — so the installer is
# always tested against the real packaging.
set -eu

if [ $# -ne 3 ]; then
  echo "usage: sh scripts/package.sh <target> <binary> <output folder>" >&2
  exit 2
fi
target="$1"
binary="$2"
root="$(cd "$(dirname "$0")/.." && pwd)"
mkdir -p "$3"
out="$(cd "$3" && pwd)"
name="hippo-task-$target"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir "$work/$name"
case "$target" in
  *windows*) exe=hippo-task.exe ;;
  *) exe=hippo-task ;;
esac
cp "$binary" "$work/$name/$exe"
chmod 755 "$work/$name/$exe"
cp "$root/LICENSE" "$root/README.md" "$work/$name/"

case "$target" in
  *windows*)
    archive="$name.zip"
    rm -f "$out/$archive"
    # GitHub's Windows machines have 7-Zip; elsewhere, `zip`.
    if command -v 7z >/dev/null 2>&1; then
      (cd "$work" && 7z a -tzip -bso0 -bsp0 "$out/$archive" "$name")
    else
      (cd "$work" && zip -qr "$out/$archive" "$name")
    fi
    ;;
  *)
    archive="$name.tar.gz"
    # COPYFILE_DISABLE: macOS's tar would otherwise add `._*` metadata files.
    (cd "$work" && COPYFILE_DISABLE=1 tar -czf "$out/$archive" "$name")
    ;;
esac

# `<sha-256>  <file name>` — the line format `shasum -c` and `sha256sum -c` read.
sha256() { if command -v sha256sum >/dev/null 2>&1; then sha256sum; else shasum -a 256; fi; }
hash="$(sha256 <"$out/$archive" | cut -d ' ' -f 1)"
printf '%s  %s\n' "$hash" "$archive" >"$out/$archive.sha256"
echo "$out/$archive"
