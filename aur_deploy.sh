#!/usr/bin/env bash
set -euo pipefail

APP_NAME="termilyon"
REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
AUR_REMOTE="${AUR_REMOTE:-https://aur.archlinux.org/termilyon.git}"
AUR_PUSH_REMOTE="${AUR_PUSH_REMOTE:-ssh://aur@aur.archlinux.org/termilyon.git}"
AUR_REPO_DIR="${AUR_REPO_DIR:-$REPO_ROOT/.aur-repo}"
PUSH_CHANGES=1

usage() {
  cat <<EOF
Usage: ./aur_deploy.sh [--no-push] [--repo-dir PATH]

Options:
  --no-push         Prepare and commit AUR changes, but do not push
  --repo-dir PATH   Override local AUR clone directory
  -h, --help        Show help

Environment:
  AUR_REMOTE        AUR git remote URL
  AUR_PUSH_REMOTE   AUR git push URL
  AUR_REPO_DIR      Local AUR clone directory
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-push)
      PUSH_CHANGES=0
      shift
      ;;
    --repo-dir)
      if [[ $# -lt 2 ]]; then
        echo "--repo-dir requires a path argument" >&2
        exit 1
      fi
      AUR_REPO_DIR="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
done

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Required command not found: $1" >&2
    exit 1
  fi
}

extract_version() {
  sed -n 's/^version = "\(.*\)"/\1/p' "$REPO_ROOT/Cargo.toml" | head -n1
}

ensure_aur_repo() {
  mkdir -p "$(dirname "$AUR_REPO_DIR")"

  if [[ -d "$AUR_REPO_DIR/.git" ]]; then
    if [[ -n "$(git -C "$AUR_REPO_DIR" status --porcelain)" ]]; then
      echo "AUR repo has uncommitted changes: $AUR_REPO_DIR" >&2
      exit 1
    fi
    git -C "$AUR_REPO_DIR" fetch origin
    git -C "$AUR_REPO_DIR" pull --ff-only origin master
    git -C "$AUR_REPO_DIR" remote set-url --push origin "$AUR_PUSH_REMOTE"
    return
  fi

  rm -rf "$AUR_REPO_DIR"
  git clone "$AUR_REMOTE" "$AUR_REPO_DIR"
  git -C "$AUR_REPO_DIR" remote set-url --push origin "$AUR_PUSH_REMOTE"
}

generate_pkgbuild() {
  local version="$1"
  local checksum="$2"

  cat > "$AUR_REPO_DIR/PKGBUILD" <<EOF
pkgname=termilyon
pkgver=$version
pkgrel=1
pkgdesc="GTK4 and VTE based terminal emulator with tabs, splits, and SSH tooling"
arch=('x86_64')
url="https://github.com/alikaya/termilyon"
license=('MIT')
depends=('gcc-libs' 'glib2' 'gtk4' 'hicolor-icon-theme' 'vte4')
makedepends=('cargo')
source=("\$pkgname-\$pkgver.tar.gz::\$url/archive/refs/tags/v\$pkgver.tar.gz")
sha256sums=('$checksum')

build() {
  cd "\$srcdir/\$pkgname-\$pkgver"
  cargo build --release
}

package() {
  cd "\$srcdir/\$pkgname-\$pkgver"

  install -Dm755 "target/release/\$pkgname" "\$pkgdir/usr/bin/\$pkgname"
  install -Dm644 "termilyon.desktop" "\$pkgdir/usr/share/applications/\$pkgname.desktop"
  install -Dm644 "logo.png" "\$pkgdir/usr/share/icons/hicolor/256x256/apps/\$pkgname.png"
  install -Dm644 "logo.png" "\$pkgdir/usr/share/pixmaps/\$pkgname.png"
}
EOF
}

ensure_ssh_key_loaded() {
  if ! ssh-add -l >/dev/null 2>&1; then
    eval "$(ssh-agent -s)"
  fi

  if [[ -f "$HOME/.ssh/aur.pub" ]]; then
    local aur_fingerprint
    aur_fingerprint="$(ssh-keygen -lf "$HOME/.ssh/aur.pub" | awk '{print $2}')"
    if ! ssh-add -l 2>/dev/null | grep -Fq "$aur_fingerprint"; then
      ssh-add "$HOME/.ssh/aur"
    fi
  else
    ssh-add "$HOME/.ssh/aur"
  fi
}

require_command git
require_command curl
require_command sha256sum
require_command makepkg
require_command ssh-add
require_command ssh-keygen
require_command sed

push_if_ahead() {
  local ahead_count
  ahead_count="$(git -C "$AUR_REPO_DIR" rev-list --count origin/master..HEAD)"
  if [[ "$ahead_count" -gt 0 ]]; then
    ensure_ssh_key_loaded
    if git -C "$AUR_REPO_DIR" push origin HEAD; then
      echo "AUR update pushed from $AUR_REPO_DIR"
      return 0
    fi

    echo "AUR push failed from $AUR_REPO_DIR" >&2
    return 1
  fi

  return 1
}

if [[ ! -f "$REPO_ROOT/Cargo.toml" ]]; then
  echo "Cargo.toml not found in $REPO_ROOT" >&2
  exit 1
fi

version="$(extract_version)"
if [[ -z "$version" ]]; then
  echo "Could not determine version from Cargo.toml" >&2
  exit 1
fi

ensure_aur_repo

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT
tarball="$tmpdir/$APP_NAME-$version.tar.gz"

echo "Downloading source tarball for v$version ..."
curl -fsSL "https://github.com/alikaya/termilyon/archive/refs/tags/v$version.tar.gz" -o "$tarball"
checksum="$(sha256sum "$tarball" | awk '{print $1}')"

echo "Generating PKGBUILD ..."
generate_pkgbuild "$version" "$checksum"

echo "Generating .SRCINFO ..."
(
  cd "$AUR_REPO_DIR"
  makepkg --printsrcinfo > .SRCINFO
)

if [[ -z "$(git -C "$AUR_REPO_DIR" status --porcelain)" ]]; then
  if [[ "$PUSH_CHANGES" -eq 1 ]] && push_if_ahead; then
    exit 0
  fi

  echo "AUR repo is already up to date."
  exit 0
fi

git -C "$AUR_REPO_DIR" add PKGBUILD .SRCINFO

if git -C "$AUR_REPO_DIR" diff --cached --quiet; then
  echo "No staged AUR changes detected."
  exit 0
fi

git -C "$AUR_REPO_DIR" commit -m "Update to v$version"

if [[ "$PUSH_CHANGES" -eq 1 ]]; then
  if ! push_if_ahead; then
    exit 1
  fi
else
  echo "AUR update committed locally in $AUR_REPO_DIR"
fi
