#!/usr/bin/env bash
set -euo pipefail

REPO="Srlion/gmod-wstool"
API="https://api.github.com/repos/${REPO}/releases/latest"

err() { echo "error: $*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1; }

# only x86_64 is published
ARCH="$(uname -m)"
[ "$ARCH" = "x86_64" ] || err "unsupported arch: $ARCH (only x86_64 is published)"

need curl || err "curl is required"

# pick installer based on available package manager
if need apt-get || need dpkg; then
    KIND="deb"
elif need dnf || need rpm; then
    KIND="rpm"
elif need pacman; then
    KIND="zst"
else
    err "no supported package manager found (apt/dnf/pacman)"
fi

echo "fetching latest release info..."
# match the asset for our package kind
case "$KIND" in
    deb) PATTERN='amd64\.deb' ;;
    rpm) PATTERN='x86_64\.rpm' ;;
    zst) PATTERN='x86_64\.pkg\.tar\.zst' ;;
esac

URL="$(curl -fsSL "$API" \
    | grep -oE '"browser_download_url": *"[^"]+"' \
    | sed -E 's/.*"(https[^"]+)"/\1/' \
    | grep -E "$PATTERN" \
    | head -n1)"

[ -n "$URL" ] || err "could not find a matching asset for $KIND"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
FILE="${TMP}/$(basename "$URL")"

echo "downloading $URL"
curl -fSL -o "$FILE" "$URL"

# need root to install
SUDO=""
if [ "$(id -u)" -ne 0 ]; then
    need sudo || err "must run as root or have sudo installed"
    SUDO="sudo"
fi

echo "installing..."
case "$KIND" in
    deb)
        if need apt-get; then
            $SUDO apt-get install -y "$FILE"
        else
            $SUDO dpkg -i "$FILE"
        fi
        ;;
    rpm)
        if need dnf; then
            $SUDO dnf install -y "$FILE"
        else
            $SUDO rpm -i "$FILE"
        fi
        ;;
    zst)
        $SUDO pacman -U --noconfirm "$FILE"
        ;;
esac

echo "done. run: gmod-wstool"
