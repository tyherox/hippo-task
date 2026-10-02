#!/bin/sh
# Install hippo-task from its GitHub releases — on macOS or Linux, no Rust needed:
#
#   curl -fsSL https://raw.githubusercontent.com/tyherox/hippo-task/main/scripts/install.sh | sh
#
# It picks the build for this machine, checks its SHA-256, and puts
# `hippo-task` next to the copy already on your PATH, or else in ~/.local/bin.
# Run it again to upgrade. Optional settings:
#   HIPPO_VERSION=v0.4.1        a specific release (default: the latest)
#   HIPPO_INSTALL_DIR=<folder>  where to put it
#   HIPPO_DOWNLOAD_BASE=<url>   where the release files are (tests use file://)
set -eu

repo="tyherox/hippo-task"
from_source="cargo install --git https://github.com/$repo --locked"
die() {
  echo "hippo-task install: $*" >&2
  exit 1
}

case "$(uname -s)" in
  Darwin) os=apple-darwin ;;
  Linux) os=unknown-linux-musl ;; # statically linked: runs on any distribution
  *) die "no installer for $(uname -s). On Windows, download the .zip from https://github.com/$repo/releases/latest — or build from source: $from_source" ;;
esac
case "$(uname -m)" in
  arm64 | aarch64) arch=aarch64 ;;
  x86_64 | amd64) arch=x86_64 ;;
  *) die "no prebuilt binary for $(uname -m) — build from source: $from_source" ;;
esac
target="$arch-$os"
asset="hippo-task-$target.tar.gz"

version="${HIPPO_VERSION:-latest}"
if [ "$version" = latest ]; then
  base="https://github.com/$repo/releases/latest/download"
else
  base="https://github.com/$repo/releases/download/$version"
fi
base="${HIPPO_DOWNLOAD_BASE:-$base}"

# Where it goes: where you asked; else next to the hippo-task already on your
# PATH (an upgrade in place); else ~/.local/bin.
dir="${HIPPO_INSTALL_DIR:-}"
if [ -z "$dir" ] && existing="$(command -v hippo-task 2>/dev/null)" && [ -w "$(dirname "$existing")" ]; then
  dir="$(dirname "$existing")"
fi
dir="${dir:-$HOME/.local/bin}"

command -v curl >/dev/null 2>&1 || die "needs curl to download the release"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "Downloading $asset ($version)…"
curl -fsSL "$base/$asset" -o "$tmp/$asset" || die "couldn't download $base/$asset"
curl -fsSL "$base/$asset.sha256" -o "$tmp/$asset.sha256" || die "couldn't download $base/$asset.sha256"

sha256() { if command -v sha256sum >/dev/null 2>&1; then sha256sum; else shasum -a 256; fi; }
expected="$(cut -d ' ' -f 1 <"$tmp/$asset.sha256")"
actual="$(sha256 <"$tmp/$asset" | cut -d ' ' -f 1)"
[ "$expected" = "$actual" ] || die "$asset doesn't match its checksum — the download is damaged, so nothing was installed. Try again."

tar -xzf "$tmp/$asset" -C "$tmp"
mkdir -p "$dir"
# Copy next to the old binary, then rename over it: a rename is atomic, and it
# works even while the old hippo-task is running.
cp "$tmp/hippo-task-$target/hippo-task" "$dir/.hippo-task.new"
chmod 755 "$dir/.hippo-task.new"
mv -f "$dir/.hippo-task.new" "$dir/hippo-task"

echo "Installed $("$dir/hippo-task" --version) in $dir"
case ":$PATH:" in
  *":$dir:"*) ;;
  *)
    echo "To use it, add $dir to your PATH — for example, in ~/.zshrc or ~/.bashrc:"
    echo "  export PATH=\"$dir:\$PATH\""
    ;;
esac
echo "Next, in a project: hippo-task init"
