#!/usr/bin/env bash
set -euo pipefail

# spawn-at bootstrap installer script
# Repository: harshp2008/spawn-at

OS="$(uname -s)"
ARCH="$(uname -m)"

if [ "$OS" != "Linux" ] || [ "$ARCH" != "x86_64" ]; then
    echo -e "\x1b[1;31mError\x1b[0m: spawn-at pre-built binaries are currently only provided for Linux x86_64 (Detected: $OS $ARCH)."
    echo "To build from source on your platform, run:"
    echo "  git clone https://github.com/harshp2008/spawn-at.git && cd spawn-at && cargo build --release"
    exit 1
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

TAR_NAME="spawn-at-x86_64-unknown-linux-gnu.tar.gz"
DOWNLOAD_URL=""

echo -e "\x1b[1;36mFetching latest release info for harshp2008/spawn-at...\x1b[0m"

# Try querying GitHub API for latest release asset URL
API_RESPONSE="$(curl -fsSL "https://api.github.com/repos/harshp2008/spawn-at/releases/latest" 2>/dev/null || true)"

if [ -n "$API_RESPONSE" ]; then
    DOWNLOAD_URL="$(echo "$API_RESPONSE" | grep "browser_download_url" | grep "$TAR_NAME" | cut -d '"' -f 4 | head -n 1 || true)"
fi

# Fallback to standard release download URL if API response didn't yield a link
if [ -z "$DOWNLOAD_URL" ]; then
    DOWNLOAD_URL="https://github.com/harshp2008/spawn-at/releases/latest/download/$TAR_NAME"
fi

echo -e "\x1b[1;36mDownloading spawn-at binary...\x1b[0m"
if ! curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/$TAR_NAME"; then
    echo -e "\x1b[1;31mError\x1b[0m: Failed to download release package from: $DOWNLOAD_URL"
    exit 1
fi

echo -e "\x1b[1;36mExtracting package...\x1b[0m"
tar -xzf "$TMP_DIR/$TAR_NAME" -C "$TMP_DIR"

if [ ! -f "$TMP_DIR/spawn-at" ]; then
    echo -e "\x1b[1;31mError\x1b[0m: Extracted package does not contain 'spawn-at' executable."
    exit 1
fi

chmod +x "$TMP_DIR/spawn-at"

echo -e "\x1b[1;32mLaunching installer...\x1b[0m\n"
exec "$TMP_DIR/spawn-at" install "$@"
