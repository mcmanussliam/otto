#!/bin/sh
set -eu

REPO="${OTTO_REPO:-mcmanussliam/otto}"
APP="otto"

usage() {
  cat <<'EOF'
Usage: install.sh [--version <tag>] [--install-dir <path>]

Options:
  --version <tag>      Install a specific version tag (for example: v1.2.3).
                       Default: latest release
  --install-dir <dir>  Install directory for the binary.
                       Default: /usr/local/bin (or ~/.local/bin fallback)
EOF
}

VERSION=""
INSTALL_DIR="${OTTO_INSTALL_DIR:-}"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      [ "$#" -ge 2 ] || { echo "missing value for --version" >&2; exit 1; }
      VERSION="$2"
      shift 2
      ;;
    --install-dir)
      [ "$#" -ge 2 ] || { echo "missing value for --install-dir" >&2; exit 1; }
      INSTALL_DIR="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "required command not found: $1" >&2
    exit 1
  }
}

need_cmd uname
need_cmd mktemp
need_cmd tar
need_cmd install

if command -v curl >/dev/null 2>&1; then
  FETCH="curl -fsSL"
elif command -v wget >/dev/null 2>&1; then
  FETCH="wget -qO-"
else
  echo "either curl or wget is required" >&2
  exit 1
fi

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
case "$OS" in
  linux) GOOS="linux" ;;
  darwin) GOOS="darwin" ;;
  *)
    echo "unsupported operating system: $OS" >&2
    exit 1
    ;;
esac

ARCH_RAW="$(uname -m)"
case "$ARCH_RAW" in
  x86_64|amd64) GOARCH="amd64" ;;
  arm64|aarch64) GOARCH="arm64" ;;
  *)
    echo "unsupported architecture: $ARCH_RAW" >&2
    exit 1
    ;;
esac

if [ -z "$VERSION" ]; then
  VERSION="$($FETCH "https://api.github.com/repos/${REPO}/releases/latest" | sed -n 's/^[[:space:]]*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
  if [ -z "$VERSION" ]; then
    echo "failed to determine latest release from GitHub API" >&2
    exit 1
  fi
fi

ASSET="${APP}_${VERSION}_${GOOS}_${GOARCH}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${VERSION}/${ASSET}"
TMP_DIR="$(mktemp -d)"
ARCHIVE_PATH="${TMP_DIR}/${ASSET}"
BIN_IN_ARCHIVE="${APP}_${VERSION}_${GOOS}_${GOARCH}"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT INT TERM

echo "Downloading ${URL}"
$FETCH "$URL" >"$ARCHIVE_PATH"

tar -xzf "$ARCHIVE_PATH" -C "$TMP_DIR"
if [ ! -f "${TMP_DIR}/${BIN_IN_ARCHIVE}" ]; then
  echo "archive did not contain expected binary: ${BIN_IN_ARCHIVE}" >&2
  exit 1
fi

if [ -z "$INSTALL_DIR" ]; then
  INSTALL_DIR="/usr/local/bin"
fi

install_local_fallback() {
  INSTALL_DIR="${HOME}/.local/bin"
  mkdir -p "$INSTALL_DIR"
  install -m 0755 "${TMP_DIR}/${BIN_IN_ARCHIVE}" "${INSTALL_DIR}/${APP}"
}

if [ -d "$INSTALL_DIR" ] && [ -w "$INSTALL_DIR" ]; then
  install -m 0755 "${TMP_DIR}/${BIN_IN_ARCHIVE}" "${INSTALL_DIR}/${APP}"
elif [ ! -e "$INSTALL_DIR" ] && mkdir -p "$INSTALL_DIR" 2>/dev/null; then
  install -m 0755 "${TMP_DIR}/${BIN_IN_ARCHIVE}" "${INSTALL_DIR}/${APP}"
elif [ "$INSTALL_DIR" = "/usr/local/bin" ] && command -v sudo >/dev/null 2>&1; then
  echo "Installing to ${INSTALL_DIR} with sudo"
  sudo mkdir -p "$INSTALL_DIR"
  sudo install -m 0755 "${TMP_DIR}/${BIN_IN_ARCHIVE}" "${INSTALL_DIR}/${APP}"
else
  echo "cannot write to ${INSTALL_DIR}, installing to ~/.local/bin instead"
  install_local_fallback
fi

echo "Installed ${APP} ${VERSION} to ${INSTALL_DIR}/${APP}"

case ":$PATH:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    echo
    echo "${INSTALL_DIR} is not on your PATH."
    echo "Add this to your shell profile:"
    echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
    ;;
esac

