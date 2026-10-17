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
#   - Unsupported desktop: --allow-unsupported installs without asking (see below)
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
yellow() { echo -e "\x1b[1;33m$1\x1b[0m"; }

# Error layout (all on stderr): a blank line, the red "Error:" headline, optional
# "  - cause" bullets, then error_help_heading and "  - advice" bullets, then error_end.
error_headline() { { echo; echo -e "\x1b[1;31mError\x1b[0m: $1"; } >&2; }
error_help_heading() { { echo; echo "What you can do:"; } >&2; }
error_end() { echo >&2; }

OS="$(uname -s)"
ARCH="$(uname -m)"

if [ "$OS" != "Linux" ] || [ "$ARCH" != "x86_64" ]; then
    error_headline "spawn-at pre-built binaries are currently only provided for Linux x86_64 (Detected: $OS $ARCH)."
    error_help_heading
    echo "  - Build from source: git clone https://github.com/${REPO}.git && cd spawn-at && cargo build --release" >&2
    error_end
    exit 1
fi

TARGET_VERSION=""
USE_PRERELEASE=false
ALLOW_UNSUPPORTED=false
EXTRA_ARGS=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        -v|--version)
            if [[ -n "${2:-}" && ! "$2" =~ ^- ]]; then
                TARGET_VERSION="$2"
                shift 2
            else
                error_headline "Flag --version / -v requires a release tag (e.g., v0.1.0 or v0.2.0-alpha)."
                error_help_heading
                echo "  - Add the tag after the flag, e.g.: bash install.sh --version v0.2.0-beta.5" >&2
                echo "  - The available tags are listed at https://github.com/${REPO}/releases" >&2
                error_end
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
        --allow-unsupported)
            # Consumed here; spawn-at itself has no such flag and never sees it.
            ALLOW_UNSUPPORTED=true
            shift
            ;;
        *)
            EXTRA_ARGS+=("$1")
            shift
            ;;
    esac
done

# --- Unsupported desktop -------------------------------------------------------------
# Decided before anything is downloaded or changed. Same rule as the spawn-at binary
# (src/platform/linux/registry.rs, `unsupported`): supported means SPAWN_AT_BACKEND forcing
# x11 or sway, a desktop containing GNOME, or an X11 session. Anything else is a non-GNOME
# Wayland (or unknown) session, where spawn-at only has an X11 fallback that may not work.
# SPAWN_AT_RESOLVE_ONLY=1 installs nothing, so it is not asked. A supported environment
# prints nothing here and the script goes on exactly as before.
DESKTOP_RAW="${XDG_CURRENT_DESKTOP:-}"
SESSION_RAW="${XDG_SESSION_TYPE:-}"
FORCED_BACKEND="$(printf '%s' "${SPAWN_AT_BACKEND:-}" | tr '[:upper:]' '[:lower:]')"
DESKTOP_UP="$(printf '%s' "$DESKTOP_RAW" | tr '[:lower:]' '[:upper:]')"
SESSION_LOW="$(printf '%s' "$SESSION_RAW" | tr '[:upper:]' '[:lower:]')"
UNSUPPORTED_ACCEPTED=false
if [ "$FORCED_BACKEND" != "x11" ] && [ "$FORCED_BACKEND" != "sway" ] \
    && [[ "$DESKTOP_UP" != *GNOME* ]] && [ "$SESSION_LOW" != "x11" ] \
    && [ "${SPAWN_AT_RESOLVE_ONLY:-}" != "1" ]; then
    echo
    yellow "Your desktop (${DESKTOP_RAW:-unknown}, ${SESSION_RAW:-unknown}) is not supported yet. spawn-at only fully supports GNOME on Wayland. An X11 fallback exists but may or may not work here."
    if [ "$ALLOW_UNSUPPORTED" = true ]; then
        UNSUPPORTED_ACCEPTED=true
        echo
    elif [ -t 2 ] && { : </dev/tty; } 2>/dev/null; then
        echo
        printf 'Install anyway? [y/N] '
        answer=""
        read -r answer </dev/tty || answer=""
        case "$answer" in
            y|Y|yes|YES|Yes) UNSUPPORTED_ACCEPTED=true ;;
            *)
                echo
                echo "Nothing was installed."
                exit 1
                ;;
        esac
    else
        echo
        echo "Nothing was installed. There is no terminal to ask on: re-run with --allow-unsupported to install anyway."
        exit 1
    fi
fi

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

#
# Progress mode (FETCH_PROGRESS=1, set only by the tarball download and only when
# stderr is a terminal): curl shows its progress bar on the terminal, so its stderr
# cannot be captured into $errf (the bar would end up in the file). Instead curl's
# error text comes from the %{errormsg} write-out variable (curl 7.75+; older curl
# gets a generic message), and retries are done here, one curl call per attempt
# with --retry 0, so that each retry can be announced.
FETCH_PROGRESS=0

# True if the installed curl supports the %{errormsg} write-out variable (7.75+).
curl_has_errormsg() {
    local v
    v="$(curl --version 2>/dev/null | sed -n '1s/^curl \([0-9]*\)\.\([0-9]*\).*/\1 \2/p')"
    [ -n "$v" ] || return 1
    set -- $v
    [ "$1" -gt 7 ] || { [ "$1" -eq 7 ] && [ "$2" -ge 75 ]; }
}

fetch() {
    local url="$1" body="$2"
    shift 2
    local hdrs="$TMP_DIR/headers.txt" errf="$TMP_DIR/curl.err" out rc=0 msg=""
    : > "$hdrs"
    : > "$errf"
    if [ "$FETCH_PROGRESS" = 1 ]; then
        local attempt=0 max_attempts=4 wfmt='%{http_code} %{url_effective}' started=$SECONDS
        curl_has_errormsg && wfmt="${wfmt}"$'\n''%{errormsg}'
        while :; do
            attempt=$((attempt + 1))
            rc=0
            out="$(curl -S --progress-bar -L --connect-timeout 10 --retry 0 \
                -D "$hdrs" -o "$body" -w "$wfmt" "$@" "$url")" || rc=$?
            msg="${out#*$'\n'}"
            [ "$msg" != "$out" ] || msg=""
            out="${out%%$'\n'*}"
            # Same cases curl's own --retry treats as transient.
            if [ "$attempt" -lt "$max_attempts" ]; then
                case "$rc:${out%% *}" in
                    28:*) echo "Download attempt ${attempt} of ${max_attempts} timed out or stalled, retrying in 2s..." >&2 ;;
                    0:408|0:429|0:500|0:502|0:503|0:504) echo "Download attempt ${attempt} of ${max_attempts} got HTTP ${out%% *}, retrying in 2s..." >&2 ;;
                    *) break ;;
                esac
                sleep 2
                continue
            fi
            break
        done
        # Old curl has no %{errormsg}; it printed its own error line on the terminal.
        [ "$rc" -eq 0 ] || [ -n "$msg" ] || msg="see the curl message above"
        if [ "$rc" -eq 0 ] && [ $((SECONDS - started)) -gt 5 ]; then
            echo "Downloaded in $((SECONDS - started))s, slow connection?" >&2
        fi
    else
        out="$(curl -sS -L --connect-timeout 10 --retry 3 --retry-delay 2 \
            -D "$hdrs" -o "$body" -w '%{http_code} %{url_effective}' \
            "$@" "$url" 2>"$errf")" || rc=$?
    fi
    FETCH_STATUS="${out%% *}"
    FETCH_FINAL_URL="${out#* }"
    [ -n "$FETCH_STATUS" ] || FETCH_STATUS="000"
    FETCH_REASON=""

    if [ "$rc" -ne 0 ]; then
        [ "$FETCH_PROGRESS" = 1 ] || msg="$(tr '\n' ' ' < "$errf")"
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
    error_headline "Could not determine which spawn-at release to install."
    for f in "${FAILURES[@]}"; do
        echo "  - ${f}" >&2
    done
    error_help_heading
    echo "  - Retry in a few minutes (rate limits and slow networks are usually temporary)." >&2
    echo "  - Set GITHUB_TOKEN to a GitHub token to raise the API rate limit, then run this script again." >&2
    echo "  - Pin a release: bash install.sh --version <tag>" >&2
    echo "  - Or download ${TAR_NAME} by hand from https://github.com/${REPO}/releases," >&2
    echo "    extract it and run: ./spawn-at install" >&2
    error_end
    exit 1
}

# --- Resolve the release tag -------------------------------------------------------
PINNED=false

if [ -n "$TARGET_VERSION" ]; then
    cyan "Resolving specified release '${TARGET_VERSION}'..."
    RESOLVED_TAG="$TARGET_VERSION"
    PINNED=true
    if ! valid_tag "$RESOLVED_TAG"; then
        error_headline "'${RESOLVED_TAG}' is not a valid release tag."
        error_help_heading
        echo "  - Use a tag as listed at https://github.com/${REPO}/releases, e.g.: bash install.sh --version v0.2.0-beta.5" >&2
        error_end
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
    local rc=0
    if [ -t 2 ]; then
        # Preflight (terminal only): ask for the first byte with the silent fetch()
        # path. A definite HTTP error status (404, 403, a 5xx that survived the retry)
        # is reported with no progress bar and no curl message. Anything else that
        # fails (timeout, DNS, connection) is ignored here: the real download below
        # has its own retries and error report. (Erasing curl's own "curl: (22)" line
        # with cursor codes was not reliable: curl prints it followed by an extra
        # blank line.) At most 2 attempts of 15 s each; later options override fetch()'s.
        if ! fetch "$url" "$TMP_DIR/preflight" -r 0-0 --max-time 15 --retry 1; then
            case "$FETCH_REASON" in
                "HTTP "*|"rate limit"*) return 1 ;;
            esac
        fi
        FETCH_PROGRESS=1
    fi
    fetch "$url" "$TMP_DIR/$TAR_NAME" --speed-time 30 --speed-limit 1000 || rc=$?
    FETCH_PROGRESS=0
    return "$rc"
}

echo
cyan "Downloading spawn-at binary (${RESOLVED_TAG})..."
if ! download_tag "$RESOLVED_TAG"; then
    first_reason="$FETCH_REASON"
    # A pinned tag given without the leading "v" (e.g. 0.2.0-beta.1) is retried with it.
    if [ "$PINNED" = true ] && [[ ! "$RESOLVED_TAG" =~ ^v ]] && download_tag "v${RESOLVED_TAG}"; then
        RESOLVED_TAG="v${RESOLVED_TAG}"
    else
        error_headline "Failed to download the release package."
        echo "  - ${first_reason}" >&2
        case "$first_reason" in
            "HTTP 404"*) echo "  - Check that release '${RESOLVED_TAG}' exists and has ${TAR_NAME}: https://github.com/${REPO}/releases" >&2 ;;
        esac
        error_help_heading
        echo "  - Retry later." >&2
        echo "  - Or download ${TAR_NAME} by hand from" >&2
        echo "    https://github.com/${REPO}/releases/download/${RESOLVED_TAG}/${TAR_NAME}" >&2
        echo "    then extract it and run: ./spawn-at install" >&2
        error_end
        exit 1
    fi
fi

echo
cyan "Extracting package..."
if ! tar -xzf "$TMP_DIR/$TAR_NAME" -C "$TMP_DIR"; then
    error_headline "The downloaded package is not a valid tar.gz archive; nothing was installed."
    error_help_heading
    echo "  - Run this script again; if it keeps failing, download ${TAR_NAME} by hand from https://github.com/${REPO}/releases" >&2
    error_end
    exit 1
fi

if [ ! -f "$TMP_DIR/spawn-at" ]; then
    error_headline "Extracted package does not contain 'spawn-at' executable."
    error_help_heading
    echo "  - Check that ${TAR_NAME} on https://github.com/${REPO}/releases contains a 'spawn-at' file, or report it as an issue." >&2
    error_end
    exit 1
fi

chmod +x "$TMP_DIR/spawn-at"

echo
echo -e "\x1b[1;32mLaunching installer...\x1b[0m\n"
# Not exec: the EXIT trap must remove the temp dir. The installed binary is only
# written by 'spawn-at install' itself, after everything above succeeded.
rc=0
if [ "$UNSUPPORTED_ACCEPTED" = true ]; then
    # The user chose the X11 fallback: the installer child gets the same opt-in the binary
    # asks for at run time, on this one process only.
    SPAWN_AT_BACKEND=x11 "$TMP_DIR/spawn-at" install "${EXTRA_ARGS[@]+"${EXTRA_ARGS[@]}"}" || rc=$?
    if [ "$rc" -eq 0 ]; then
        echo
        yellow "Reminder: on this desktop spawn-at only runs with SPAWN_AT_BACKEND=x11 (for example: export SPAWN_AT_BACKEND=x11 in your shell profile). It may or may not work."
    fi
else
    "$TMP_DIR/spawn-at" install "${EXTRA_ARGS[@]+"${EXTRA_ARGS[@]}"}" || rc=$?
fi
exit "$rc"
