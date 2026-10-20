# shellcheck shell=bash
# Shared setup for the packaging scripts. Source it: `. "$(dirname "$0")/../env.sh"`.
#
# Exports:
#   ROOT                    workspace root
#   VERSION                 [workspace.package] version from Cargo.toml (override: WORDCRAFT_VERSION)
#   DIST                    output directory for release artifacts (default: $ROOT/dist/release)
#   WORDCRAFT_BUILD_SHA    git commit baked into the binaries (see crates/engine/src/build_info.rs)
#   WORDCRAFT_BUILD_DATE   UTC build date, YYYY-MM-DD
#   CARGO_TARGET_DIR        cargo's target dir (default: $ROOT/target)

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export ROOT

# The version lives in exactly one place: `[workspace.package] version` in the root Cargo.toml.
# (`cargo xtask version` prints the same thing; awk avoids compiling xtask here.)
workspace_version() {
  awk '
    /^\[/ { in_pkg = ($0 == "[workspace.package]") ; next }
    in_pkg && $1 == "version" { gsub(/[" ]/, "", $3); print $3; exit }
  ' "$ROOT/Cargo.toml"
}

VERSION="${WORDCRAFT_VERSION:-$(workspace_version)}"
if [ -z "$VERSION" ]; then
  echo "error: could not read [workspace.package] version from $ROOT/Cargo.toml" >&2
  exit 1
fi
export VERSION

DIST="${DIST:-$ROOT/dist/release}"
mkdir -p "$DIST"
export DIST

if [ -z "${WORDCRAFT_BUILD_SHA:-}" ]; then
  WORDCRAFT_BUILD_SHA="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || true)"
fi
export WORDCRAFT_BUILD_SHA
export WORDCRAFT_BUILD_DATE="${WORDCRAFT_BUILD_DATE:-$(date -u +%Y-%m-%d)}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"

# Emit a GitHub Actions warning (plain stderr outside Actions).
warn() {
  if [ -n "${GITHUB_ACTIONS:-}" ]; then echo "::warning::$*"; else echo "warning: $*" >&2; fi
}

# Copy licence and readme files that exist into a package directory.
copy_docs() {
  local dest="$1" f
  for f in README.md LICENSE LICENSE-MIT LICENSE-APACHE COPYRIGHT; do
    if [ -f "$ROOT/$f" ]; then cp "$ROOT/$f" "$dest/"; fi
  done
  copy_font_licences "$dest"
}

# Builds made with craft-fonts (CRAFT_FONTS_DIR, set by the release workflow) embed its fonts, so
# the package carries their licences as OFL-<family-dir>.txt. Nothing without CRAFT_FONTS_DIR.
copy_font_licences() {
  local dest="$1" f
  [ -n "${CRAFT_FONTS_DIR:-}" ] || return 0
  for f in "$CRAFT_FONTS_DIR"/fonts/*/OFL.txt; do
    if [ -f "$f" ]; then cp "$f" "$dest/OFL-$(basename "$(dirname "$f")").txt"; fi
  done
}

# Plume: the TeX engine its packages ship, for PDF export through a template (Tectonic, MIT;
# https://tectonic-typesetting.github.io). Override the version with TECTONIC_VERSION, or give a
# binary with TECTONIC_BIN (one platform's) to skip the download.
TECTONIC_VERSION="${TECTONIC_VERSION:-0.15.0}"

# fetch_tectonic <rust-target> <dest-file>: Tectonic's release binary for that target at
# <dest-file>. Warns and returns 1 when there's none (a platform Tectonic doesn't build for, no
# network): the package is then made without it, and Plume uses an installed TeX engine.
fetch_tectonic() {
  local target="$1" dest="$2" ext=tar.gz tmp url bin
  if [ -n "${TECTONIC_BIN:-}" ] && [ -f "$TECTONIC_BIN" ]; then
    install -m755 -D "$TECTONIC_BIN" "$dest" 2>/dev/null || { mkdir -p "$(dirname "$dest")" && cp "$TECTONIC_BIN" "$dest" && chmod 755 "$dest"; }
    return 0
  fi
  case "$target" in
    *-windows-*) ext=zip ;;
    *-linux-gnu) target="${target%-gnu}-musl" ;;
  esac
  url="https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic%40${TECTONIC_VERSION}/tectonic-${TECTONIC_VERSION}-${target}.${ext}"
  tmp="$(mktemp -d)"
  if ! curl -fsSL -o "$tmp/tectonic.$ext" "$url"; then
    warn "no Tectonic $TECTONIC_VERSION for $target ($url): this package has no PDF engine of its own"
    rm -rf "$tmp"
    return 1
  fi
  if [ "$ext" = zip ]; then unzip -q -o "$tmp/tectonic.zip" -d "$tmp"; else tar -xzf "$tmp/tectonic.$ext" -C "$tmp"; fi
  bin="$(find "$tmp" -type f \( -name tectonic -o -name tectonic.exe \) | head -n 1)"
  if [ -z "$bin" ]; then
    warn "the Tectonic $TECTONIC_VERSION archive for $target has no tectonic binary"
    rm -rf "$tmp"
    return 1
  fi
  mkdir -p "$(dirname "$dest")"
  cp "$bin" "$dest"
  chmod 755 "$dest"
  rm -rf "$tmp"
}

# Portable SHA-256 of a file (prints just the hash).
sha256() {
  if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}
