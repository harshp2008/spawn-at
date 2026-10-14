#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# spawn-at Remote Bootstrap Installer
# Repository: harshp2008/spawn-at
#
# Supports:
#   - Default: Latest STABLE release
#   - Pinning: --version <tag> or -v <tag>
#   - Prerelease: --prerelease (latest release including beta/alphas)
#   - Execution: Pass-through of flags to 'spawn-at install "$@"'
#
# Release resolution:
#   1. GitHub API (retried; sends GITHUB_TOKEN as a Bearer token if it is set).
#   2. If that fails: GitHub's web endpoints, which are not API-rate-limited:
#      the releases.atom feed (--prerelease) or the redirect of /releases/latest
#      (stable).
#   The tarball itself always comes from the plain releases/download URL.
#
# Environment:
#   GITHUB_TOKEN             optional, raises the API rate limit
#   SPAWN_AT_RESOLVE_ONLY=1  print the resolved tag and exit (no download)
# ==============================================================================

REPO="harshp2008/spawn-at"
TAR_NAME="spawn-at-x86_64-unknown-linux-gnu.tar.gz"

red() { echo -e "\x1b[1;31m$1\x1b[0m"; }
cyan() { echo -e "\x1b[1;36m$1\x1b[0m"; }

OS="$(uname -s)"
ARCH="$(uname -m)"

if [ "$OS" != "Linux" ] || [ "$ARCH" != "x86_64" ]; then
    echo -e "\x1b[1;31mError\x1b[0m: spawn-at pre-built binaries are currently only provided for Linux x86_64 (Detected: $OS $ARCH)."
    echo "To build from source on your platform, run:"
    echo "  git clone https://github.com/${REPO}.git && cd spawn-at && cargo build --release"
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

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

# --- HTTP helper --------------------------------------------------------------
# fetch URL BODY_FILE [extra curl options...]
# Sets FETCH_STATUS (HTTP code, "000" if no response), FETCH_REASON (empty on
# success, else a human-readable cause) and FETCH_FINAL_URL. Returns 0 only for
# a 2xx response.
FETCH_STATUS=""
FETCH_REASON=""
FETCH_FINAL_URL=""

fetch() {
    local url="$1" body="$2"
    shift 2
    local hdrs="$TMP_DIR/headers.txt" errf="$TMP_DIR/curl.err" out rc=0
    : > "$hdrs"
    : > "$errf"
    out="$(curl -sS -L --connect-timeout 10 --retry 3 --retry-delay 2 \
        -D "$hdrs" -o "$body" -w '%{http_code} %{url_effective}' \
        "$@" "$url" 2>"$errf")" || rc=$?
    FETCH_STATUS="${out%% *}"
    FETCH_FINAL_URL="${out#* }"
    [ -n "$FETCH_STATUS" ] || FETCH_STATUS="000"
    FETCH_REASON=""

    if [ "$rc" -ne 0 ]; then
        local msg
        msg="$(tr '\n' ' ' < "$errf")"
        case "$rc" in
            28) FETCH_REASON="timed out talking to ${url} (curl exit 28)" ;;
            6|7) FETCH_REASON="no network or DNS failure reaching ${url} (curl exit ${rc}: ${msg})" ;;
            *) FETCH_REASON="curl failed for ${url} (exit ${rc}: ${msg})" ;;
        esac
        return 1
    fi

    case "$FETCH_STATUS" in
        2??) return 0 ;;
        403|429)
            if grep -qi '^x-ratelimit-remaining: *0' "$hdrs" || grep -qi 'rate limit' "$body" 2>/dev/null; then
                FETCH_REASON="rate limit exceeded (HTTP ${FETCH_STATUS}; unauthenticated clients get 60 requests per hour per IP address)"
            else
                FETCH_REASON="HTTP ${FETCH_STATUS} from ${url}"
            fi
            ;;
        *) FETCH_REASON="HTTP ${FETCH_STATUS} from ${url}" ;;
    esac
    return 1
}

# --- Tag extraction (jq if present, otherwise grep and sed) -------------------------
# First non-draft tag_name from a GitHub API JSON body (object or array).
json_tag() {
    local file="$1"
    if command -v jq >/dev/null 2>&1; then
        jq -r 'if type == "array" then map(select(.draft == false))[0].tag_name // empty else .tag_name // empty end' "$file" 2>/dev/null | sed -n '1p'
    else
        # grep exits 1 on "no match", which is not an error here.
        { grep -o '"tag_name"[[:space:]]*:[[:space:]]*"[^"]*"' "$file" || test $? -eq 1; } \
            | sed -n '1s/.*:[[:space:]]*"\(.*\)"$/\1/p'
    fi
}

# Tag of the first <entry> of a releases.atom feed.
atom_tag() {
    local file="$1"
    { grep -o 'href="[^"]*/releases/tag/[^"]*"' "$file" || test $? -eq 1; } \
        | sed -n '1s|.*/releases/tag/||; 1s|"$||; 1p'
}

# Only plain tag characters may reach a URL.
valid_tag() {
    [[ "$1" =~ ^[A-Za-z0-9][A-Za-z0-9._+-]*$ ]]
}

FAILURES=()
note_failure() { FAILURES+=("$1"); }

RESOLVED_TAG=""

# The resolve_via_* functions set RESOLVED_TAG and return 0, or record why
# they failed (note_failure) and return 1. They are never run in a subshell.
resolve_via_api() {
    local path="$1" body="$TMP_DIR/api.json" auth=() tag
    if [ -n "${GITHUB_TOKEN:-}" ]; then
        auth=(-H "Authorization: Bearer ${GITHUB_TOKEN}")
    fi
    if ! fetch "https://api.github.com/repos/${REPO}/${path}" "$body" --max-time 60 \
        -H "User-Agent: spawn-at-installer" -H "Accept: application/vnd.github+json" "${auth[@]+"${auth[@]}"}"; then
        note_failure "GitHub API: ${FETCH_REASON}"
        return 1
    fi
    tag="$(json_tag "$body")"
    if [ -z "$tag" ] || ! valid_tag "$tag"; then
        note_failure "GitHub API: HTTP ${FETCH_STATUS} but the response had no usable release tag"
        return 1
    fi
    RESOLVED_TAG="$tag"
}

resolve_via_atom() {
    local body="$TMP_DIR/releases.atom" tag
    if ! fetch "https://github.com/${REPO}/releases.atom" "$body" --max-time 60; then
        note_failure "releases.atom: ${FETCH_REASON}"
        return 1
    fi
    tag="$(atom_tag "$body")"
    if [ -z "$tag" ] || ! valid_tag "$tag"; then
        note_failure "releases.atom: HTTP ${FETCH_STATUS} but the feed had no release entry"
        return 1
    fi
    RESOLVED_TAG="$tag"
}

resolve_via_redirect() {
    local body="$TMP_DIR/latest.html" tag
    if ! fetch "https://github.com/${REPO}/releases/latest" "$body" --max-time 60; then
        note_failure "releases/latest redirect: ${FETCH_REASON}"
        return 1
    fi
    case "$FETCH_FINAL_URL" in
        */releases/tag/*) tag="${FETCH_FINAL_URL##*/releases/tag/}" ;;
        *) tag="" ;;
    esac
    if [ -z "$tag" ] || ! valid_tag "$tag"; then
        note_failure "releases/latest redirect: ended at ${FETCH_FINAL_URL}, which is not a release page"
        return 1
    fi
    RESOLVED_TAG="$tag"
}

fail_resolution() {
    local f
    echo -e "\x1b[1;31mError\x1b[0m: Could not determine which spawn-at release to install." >&2
    for f in "${FAILURES[@]}"; do
        echo "  - ${f}" >&2
    done
    echo "What you can do:" >&2
    echo "  - Retry in a few minutes (rate limits and slow networks are usually temporary)." >&2
    echo "  - Set GITHUB_TOKEN to a GitHub token to raise the API rate limit, then run this script again." >&2
    echo "  - Pin a release: bash install.sh --version <tag>" >&2
    echo "  - Or download ${TAR_NAME} by hand from https://github.com/${REPO}/releases," >&2
    echo "    extract it and run: ./spawn-at install" >&2
    exit 1
}

# --- Resolve the release tag -------------------------------------------------------
PINNED=false

if [ -n "$TARGET_VERSION" ]; then
    cyan "Resolving specified release '${TARGET_VERSION}'..."
    RESOLVED_TAG="$TARGET_VERSION"
    PINNED=true
    if ! valid_tag "$RESOLVED_TAG"; then
        echo -e "\x1b[1;31mError\x1b[0m: '${RESOLVED_TAG}' is not a valid release tag." >&2
        exit 1
    fi
elif [ "$USE_PRERELEASE" = true ]; then
    cyan "Fetching latest release (including pre-releases)..."
    if ! resolve_via_api "releases?per_page=10"; then
        cyan "${FAILURES[${#FAILURES[@]}-1]}; trying releases.atom..."
        resolve_via_atom || fail_resolution
    fi
else
    cyan "Fetching latest stable release info..."
    if ! resolve_via_api "releases/latest"; then
        cyan "${FAILURES[${#FAILURES[@]}-1]}; trying the releases/latest redirect..."
        resolve_via_redirect || fail_resolution
    fi
fi

if [ "${SPAWN_AT_RESOLVE_ONLY:-}" = "1" ]; then
    echo "$RESOLVED_TAG"
    exit 0
fi

# --- Download (plain release URL, never the API) -------------------------------------
download_tag() {
    local tag="$1" url="https://github.com/${REPO}/releases/download/${1}/${TAR_NAME}"
    fetch "$url" "$TMP_DIR/$TAR_NAME" --speed-time 30 --speed-limit 1000
}

cyan "Downloading spawn-at binary (${RESOLVED_TAG})..."
if ! download_tag "$RESOLVED_TAG"; then
    first_reason="$FETCH_REASON"
    # A pinned tag given without the leading "v" (e.g. 0.2.0-beta.1) is retried with it.
    if [ "$PINNED" = true ] && [[ ! "$RESOLVED_TAG" =~ ^v ]] && download_tag "v${RESOLVED_TAG}"; then
        RESOLVED_TAG="v${RESOLVED_TAG}"
    else
        echo -e "\x1b[1;31mError\x1b[0m: Failed to download the release package." >&2
        echo "  - ${first_reason}" >&2
        case "$first_reason" in
            "HTTP 404"*) echo "  - Check that release '${RESOLVED_TAG}' exists and has ${TAR_NAME}: https://github.com/${REPO}/releases" >&2 ;;
        esac
        echo "What you can do: retry later, or download ${TAR_NAME} by hand from" >&2
        echo "  https://github.com/${REPO}/releases/download/${RESOLVED_TAG}/${TAR_NAME}" >&2
        echo "then extract it and run: ./spawn-at install" >&2
        exit 1
    fi
fi

cyan "Extracting package..."
if ! tar -xzf "$TMP_DIR/$TAR_NAME" -C "$TMP_DIR"; then
    echo -e "\x1b[1;31mError\x1b[0m: The downloaded package is not a valid tar.gz archive; nothing was installed." >&2
    exit 1
fi

if [ ! -f "$TMP_DIR/spawn-at" ]; then
    echo -e "\x1b[1;31mError\x1b[0m: Extracted package does not contain 'spawn-at' executable."
    exit 1
fi

chmod +x "$TMP_DIR/spawn-at"

echo -e "\x1b[1;32mLaunching installer...\x1b[0m\n"
# Not exec: the EXIT trap must remove the temp dir. The installed binary is only
# written by 'spawn-at install' itself, after everything above succeeded.
rc=0
"$TMP_DIR/spawn-at" install "${EXTRA_ARGS[@]+"${EXTRA_ARGS[@]}"}" || rc=$?
exit "$rc"
