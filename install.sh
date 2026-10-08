#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# spawn-at Remote Bootstrap Installer
# Repository: harshp2008/spawn-at
#
# Supports:
#   - Default: Latest STABLE release (/releases/latest)
#   - Pinning: --version <tag> or -v <tag>
#   - Prerelease: --prerelease (fetches latest release including beta/alphas)
#   - Execution: Pass-through of flags to 'spawn-at install "$@"'
# ==============================================================================

OS="$(uname -s)"
ARCH="$(uname -m)"

if [ "$OS" != "Linux" ] || [ "$ARCH" != "x86_64" ]; then
    echo -e "\x1b[1;31mError\x1b[0m: spawn-at pre-built binaries are currently only provided for Linux x86_64 (Detected: $OS $ARCH)."
    echo "To build from source on your platform, run:"
    echo "  git clone https://github.com/harshp2008/spawn-at.git && cd spawn-at && cargo build --release"
    exit 1
fi

TARGET_VERSION=""
USE_PRERELEASE=false
EXTRA_ARGS=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        -v|--version)
            if [[ -n "${2:-}" && ! "$2" =~ ^- ]]; then
                TARGET_VERSION="$2"
                shift 2
            else
                echo -e "\x1b[1;31mError\x1b[0m: Flag --version / -v requires a release tag (e.g., v0.1.0 or v0.2.0-alpha)."
                exit 1
            fi
            ;;
        --version=*)
            TARGET_VERSION="${1#*=}"
            shift
            ;;
        --prerelease)
            USE_PRERELEASE=true
            shift
            ;;
        *)
            EXTRA_ARGS+=("$1")
            shift
            ;;
    esac
done

TAR_NAME="spawn-at-x86_64-unknown-linux-gnu.tar.gz"
DOWNLOAD_URL=""
RESOLVED_TAG=""

extract_asset_url() {
    local json="$1"
    local asset_name="$2"

    if command -v jq >/dev/null 2>&1; then
        echo "$json" | jq -r --arg name "$asset_name" '
            if type == "array" then
                map(select(.draft == false))[0].assets[]? | select(.name == $name) | .browser_download_url
            else
                .assets[]? | select(.name == $name) | .browser_download_url
            end
        ' | head -n 1
    elif command -v python3 >/dev/null 2>&1; then
        echo "$json" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    asset_name = sys.argv[1]
    if isinstance(data, list):
        filtered = [r for r in data if not r.get('draft')]
        data = filtered[0] if filtered else {}
    for a in data.get('assets', []):
        if a.get('name') == asset_name:
            print(a.get('browser_download_url', ''))
            break
except Exception:
    pass
" "$asset_name" 2>/dev/null || true
    else
        echo "$json" | grep "browser_download_url" | grep "$asset_name" | cut -d '"' -f 4 | head -n 1 || true
    fi
}

extract_tag_name() {
    local json="$1"

    if command -v jq >/dev/null 2>&1; then
        echo "$json" | jq -r '
            if type == "array" then
                map(select(.draft == false))[0].tag_name // empty
            else
                .tag_name // empty
            end
        ' | head -n 1
    elif command -v python3 >/dev/null 2>&1; then
        echo "$json" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    if isinstance(data, list):
        filtered = [r for r in data if not r.get('draft')]
        data = filtered[0] if filtered else {}
    print(data.get('tag_name', ''))
except Exception:
    pass
" 2>/dev/null || true
    else
        echo "$json" | grep "tag_name" | cut -d '"' -f 4 | head -n 1 || true
    fi
}

# Determine download source based on requested version and flags
if [ -n "$TARGET_VERSION" ]; then
    echo -e "\x1b[1;36mResolving specified release '${TARGET_VERSION}'...\x1b[0m"
    RESOLVED_TAG="$TARGET_VERSION"

    # Query specific tag via GitHub API
    API_RESPONSE="$(curl -fsSL --connect-timeout 8 -H "User-Agent: spawn-at-installer" "https://api.github.com/repos/harshp2008/spawn-at/releases/tags/${TARGET_VERSION}" 2>/dev/null || true)"
    if [ -z "$API_RESPONSE" ] && [[ ! "$TARGET_VERSION" =~ ^v ]]; then
        API_RESPONSE="$(curl -fsSL --connect-timeout 8 -H "User-Agent: spawn-at-installer" "https://api.github.com/repos/harshp2008/spawn-at/releases/tags/v${TARGET_VERSION}" 2>/dev/null || true)"
        if [ -n "$API_RESPONSE" ]; then
            RESOLVED_TAG="v${TARGET_VERSION}"
        fi
    fi

    if [ -n "$API_RESPONSE" ]; then
        DOWNLOAD_URL="$(extract_asset_url "$API_RESPONSE" "$TAR_NAME")"
    fi

    if [ -z "$DOWNLOAD_URL" ]; then
        DOWNLOAD_URL="https://github.com/harshp2008/spawn-at/releases/download/${RESOLVED_TAG}/${TAR_NAME}"
    fi

elif [ "$USE_PRERELEASE" = true ]; then
    echo -e "\x1b[1;36mFetching latest release (including pre-releases)...\x1b[0m"
    API_RESPONSE="$(curl -fsSL --connect-timeout 8 -H "User-Agent: spawn-at-installer" "https://api.github.com/repos/harshp2008/spawn-at/releases" 2>/dev/null || true)"

    if [ -n "$API_RESPONSE" ]; then
        RESOLVED_TAG="$(extract_tag_name "$API_RESPONSE")"
        DOWNLOAD_URL="$(extract_asset_url "$API_RESPONSE" "$TAR_NAME")"
    fi

    if [ -z "$DOWNLOAD_URL" ]; then
        echo -e "\x1b[1;31mError\x1b[0m: Could not resolve latest release via GitHub API."
        exit 1
    fi

else
    echo -e "\x1b[1;36mFetching latest stable release info...\x1b[0m"
    API_RESPONSE="$(curl -fsSL --connect-timeout 8 -H "User-Agent: spawn-at-installer" "https://api.github.com/repos/harshp2008/spawn-at/releases/latest" 2>/dev/null || true)"

    if [ -n "$API_RESPONSE" ]; then
        RESOLVED_TAG="$(extract_tag_name "$API_RESPONSE")"
        DOWNLOAD_URL="$(extract_asset_url "$API_RESPONSE" "$TAR_NAME")"
    fi

    if [ -z "$DOWNLOAD_URL" ]; then
        RESOLVED_TAG="latest"
        DOWNLOAD_URL="https://github.com/harshp2008/spawn-at/releases/latest/download/$TAR_NAME"
    fi
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

echo -e "\x1b[1;36mDownloading spawn-at binary (${RESOLVED_TAG:-latest})...\x1b[0m"
if [ -t 1 ]; then
    CURL_PROGRESS=("-#")
else
    CURL_PROGRESS=("-sSL")
fi

if ! curl "${CURL_PROGRESS[@]}" -L -f "$DOWNLOAD_URL" -o "$TMP_DIR/$TAR_NAME"; then
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
exec "$TMP_DIR/spawn-at" install "${EXTRA_ARGS[@]}"
