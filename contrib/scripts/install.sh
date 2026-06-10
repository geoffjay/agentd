#!/bin/sh
# agentd installer
#
# Downloads the agentd release tarball for this platform from GitHub Releases,
# verifies its checksum, and runs `agent install` to perform the
# platform-specific setup (binaries, launchd/systemd services, config,
# database migrations). All installation logic lives in the Rust `agent`
# binary; this script only fetches and verifies the artifacts.
#
# Usage:
#   curl -fsSL https://github.com/geoffjay/agentd/releases/latest/download/install.sh | sh
#
# Environment variables:
#   AGENTD_VERSION  Install a specific version (e.g. "v0.5.0" or "0.5.0")
#                   instead of the latest release.
#   PREFIX          Install prefix honoured by `agent install`
#                   (default: /usr/local on macOS or as root, ~/.local otherwise).
#
# The script is non-interactive and safe to pipe to sh: `agent install` reads
# nothing from stdin.

set -eu

REPO="geoffjay/agentd"

info() { printf '\033[0;34m==>\033[0m %s\n' "$1"; }
error() { printf '\033[0;31merror:\033[0m %s\n' "$1" >&2; exit 1; }

# Map uname output to a release target triple.
detect_target() {
    os=$(uname -s)
    arch=$(uname -m)
    case "$os/$arch" in
        Linux/x86_64) echo "x86_64-unknown-linux-musl" ;;
        Linux/aarch64 | Linux/arm64) echo "aarch64-unknown-linux-musl" ;;
        Darwin/x86_64) echo "x86_64-apple-darwin" ;;
        Darwin/arm64) echo "aarch64-apple-darwin" ;;
        *)
            error "unsupported platform: $os/$arch
Supported platforms:
  Linux   x86_64, aarch64 (static musl binaries)
  macOS   x86_64 (Intel), arm64 (Apple Silicon)"
            ;;
    esac
}

# Download $1 to $2 using curl or wget.
download() {
    url=$1
    dest=$2
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL -o "$dest" "$url" || error "download failed: $url"
    elif command -v wget >/dev/null 2>&1; then
        wget -q -O "$dest" "$url" || error "download failed: $url"
    else
        error "neither curl nor wget is available"
    fi
}

# Resolve the release tag: $AGENTD_VERSION if set, otherwise the tag the
# `releases/latest` redirect points at (avoids the GitHub API and its rate
# limits on the happy path).
resolve_version() {
    if [ -n "${AGENTD_VERSION:-}" ]; then
        case "$AGENTD_VERSION" in
            v*) echo "$AGENTD_VERSION" ;;
            *) echo "v$AGENTD_VERSION" ;;
        esac
        return
    fi
    latest_url="https://github.com/$REPO/releases/latest"
    if command -v curl >/dev/null 2>&1; then
        effective=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "$latest_url") ||
            error "failed to resolve the latest release from $latest_url"
    elif command -v wget >/dev/null 2>&1; then
        effective=$(wget -q -S --max-redirect=0 -O /dev/null "$latest_url" 2>&1 |
            sed -n 's/^ *[Ll]ocation: *//p' | tr -d '\r' | head -n 1) ||
            true
        [ -n "$effective" ] || error "failed to resolve the latest release from $latest_url"
    else
        error "neither curl nor wget is available"
    fi
    version=${effective##*/tag/}
    case "$version" in
        v*) echo "$version" ;;
        *) error "could not determine the latest release tag (got '$effective')" ;;
    esac
}

verify_checksum() {
    dir=$1
    asset=$2
    (
        cd "$dir"
        grep " ${asset}\$" SHA256SUMS > checksum.txt ||
            error "no checksum entry for $asset in SHA256SUMS"
        if command -v sha256sum >/dev/null 2>&1; then
            sha256sum -c checksum.txt >/dev/null 2>&1 ||
                error "checksum verification failed for $asset"
        elif command -v shasum >/dev/null 2>&1; then
            shasum -a 256 -c checksum.txt >/dev/null 2>&1 ||
                error "checksum verification failed for $asset"
        else
            error "neither sha256sum nor shasum is available"
        fi
    )
}

main() {
    target=$(detect_target)
    version=$(resolve_version)
    asset="agentd-${version}-${target}.tar.gz"
    base="https://github.com/$REPO/releases/download/$version"

    # Explicit template: unlike `mktemp -d` bare, this honours $TMPDIR on
    # macOS as well as Linux.
    tmpdir=$(mktemp -d "${TMPDIR:-/tmp}/agentd-install.XXXXXX")
    trap 'rm -rf "$tmpdir"' EXIT INT TERM

    info "Installing agentd $version for $target"

    info "Downloading $asset"
    download "$base/$asset" "$tmpdir/$asset"
    download "$base/SHA256SUMS" "$tmpdir/SHA256SUMS"

    info "Verifying checksum"
    verify_checksum "$tmpdir" "$asset"

    info "Extracting"
    stage="$tmpdir/stage"
    mkdir "$stage"
    tar -xzf "$tmpdir/$asset" -C "$stage"
    # curl/wget downloads carry no quarantine attribute, but clear it
    # defensively for copies fetched via a browser on macOS.
    if command -v xattr >/dev/null 2>&1; then
        xattr -dr com.apple.quarantine "$stage" 2>/dev/null || true
    fi

    info "Running agent install"
    "$stage/agent" install --bin-src "$stage" --ui-dir "$stage/ui"

    info "Done. Next steps:"
    echo "  - ensure the install bin directory is on your PATH" \
        "(/usr/local/bin, or ~/.local/bin for a Linux user install)"
    echo "  - start the services: agent service start"
    echo "  - check status:       agent service status"
}

main
