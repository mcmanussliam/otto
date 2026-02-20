#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
  echo "usage: $0 <tag> [output-path]" >&2
  exit 1
fi

TAG="$1"
OUT_PATH="${2:-Formula/otto.rb}"
REPO="${REPO:-mcmanussliam/otto}"

if [[ ! "$TAG" =~ ^v[0-9].* ]]; then
  echo "tag must start with v (example: v1.2.3)" >&2
  exit 1
fi

VERSION="${TAG#v}"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

SRC_URL="https://github.com/${REPO}/archive/refs/tags/${TAG}.tar.gz"
SRC_ARCHIVE="${TMP_DIR}/source.tar.gz"

curl -fsSL "$SRC_URL" -o "$SRC_ARCHIVE"
SHA256="$(shasum -a 256 "$SRC_ARCHIVE" | awk '{print $1}')"

mkdir -p "$(dirname "$OUT_PATH")"
cat >"$OUT_PATH" <<EOF
class Otto < Formula
  desc "Task runner with retries, timeouts, history, and notifications"
  homepage "https://github.com/${REPO}"
  url "${SRC_URL}"
  sha256 "${SHA256}"
  license "MIT"
  version "${VERSION}"

  depends_on "go" => :build

  def install
    ldflags = %W[
      -s -w
      -X github.com/mcmanussliam/otto/internal/version.Value=v#{version}
    ]

    system "go", "build", *std_go_args(ldflags: ldflags), "./cmd/otto"
  end

  test do
    assert_match(/[[:graph:]]+/, shell_output("#{bin}/otto version"))
  end
end
EOF

echo "Rendered ${OUT_PATH} for ${TAG}"
