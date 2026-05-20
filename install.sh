#!/usr/bin/env bash
set -euo pipefail

APP_NAME="termilyon"
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$SCRIPT_DIR"
BUILD_PROFILE="release"
MODE="user"

usage() {
  cat <<'EOF'
Usage: ./install.sh [--user|--system] [--debug]

Options:
  --user    Install into ~/.local (default)
  --system  Install into /usr/local and /usr/share (requires root/sudo)
  --debug   Build debug binary instead of release
  -h, --help  Show help
EOF
}

for arg in "$@"; do
  case "$arg" in
    --user) MODE="user" ;;
    --system) MODE="system" ;;
    --debug) BUILD_PROFILE="debug" ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $arg"
      usage
      exit 1
      ;;
  esac
done

if [[ ! -f "$REPO_DIR/Cargo.toml" ]]; then
  echo "Cargo.toml not found in $REPO_DIR"
  exit 1
fi

if [[ ! -f "$REPO_DIR/termilyon.desktop" ]]; then
  echo "termilyon.desktop not found in $REPO_DIR"
  exit 1
fi

if [[ ! -f "$REPO_DIR/logo.png" ]]; then
  echo "logo.png not found in $REPO_DIR"
  exit 1
fi

cd "$REPO_DIR"

if [[ "$BUILD_PROFILE" == "release" ]]; then
  echo "Building release binary..."
  cargo build --release
  BIN_PATH="$REPO_DIR/target/release/$APP_NAME"
else
  echo "Building debug binary..."
  cargo build
  BIN_PATH="$REPO_DIR/target/debug/$APP_NAME"
fi

if [[ ! -x "$BIN_PATH" ]]; then
  echo "Built binary not found: $BIN_PATH"
  exit 1
fi

if [[ "$MODE" == "user" ]]; then
  BIN_DIR="${HOME}/.local/bin"
  APP_DIR="${HOME}/.local/share/applications"
  ICON_DIR="${HOME}/.local/share/icons/hicolor/256x256/apps"
  PIXMAP_DIR="${HOME}/.local/share/pixmaps"
  INSTALL_CMD="install"
else
  BIN_DIR="/usr/local/bin"
  APP_DIR="/usr/share/applications"
  ICON_DIR="/usr/share/icons/hicolor/256x256/apps"
  PIXMAP_DIR="/usr/share/pixmaps"
  if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
    echo "--system mode requires root. Re-run with sudo."
    exit 1
  fi
  INSTALL_CMD="install"
fi

echo "Installing binary to $BIN_DIR ..."
mkdir -p "$BIN_DIR" "$APP_DIR" "$ICON_DIR" "$PIXMAP_DIR"
$INSTALL_CMD -m 0755 "$BIN_PATH" "$BIN_DIR/$APP_NAME"

echo "Installing desktop entry to $APP_DIR ..."
$INSTALL_CMD -m 0644 "$REPO_DIR/termilyon.desktop" "$APP_DIR/$APP_NAME.desktop"

echo "Installing icon to $ICON_DIR and $PIXMAP_DIR ..."
$INSTALL_CMD -m 0644 "$REPO_DIR/logo.png" "$ICON_DIR/$APP_NAME.png"
$INSTALL_CMD -m 0644 "$REPO_DIR/logo.png" "$PIXMAP_DIR/$APP_NAME.png"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -q "${ICON_DIR%/256x256/apps}" >/dev/null 2>&1 || true
fi

echo "Install complete."
echo "Mode: $MODE"
echo "Binary: $BIN_DIR/$APP_NAME"
echo "Desktop file: $APP_DIR/$APP_NAME.desktop"
echo "Icon: $ICON_DIR/$APP_NAME.png"
