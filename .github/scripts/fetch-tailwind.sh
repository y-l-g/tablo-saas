#!/usr/bin/env bash
# Puts the Tailwind CLI for PLATFORM in DIR under the name topcoat's build cache gives it.
#
# GitHub's release download links answer 504 at times while its API still serves the asset, and
# topcoat reuses a CLI already in its cache, so CI fetches it through the API first. Keep the
# version and digests at topcoat's `DEFAULT_VERSION`: on a mismatch the build downloads its own
# and this script only wastes time.
#
# usage: fetch-tailwind.sh PLATFORM DIR    (PLATFORM: linux-x64 or linux-arm64; needs GH_TOKEN)
set -euo pipefail

version=4.3.2
platform=$1
dir=$2
case "$platform" in
    linux-x64) sha256=5036c4fb4328e0bcdbb6065c70d8ac9452e0d4c947113a788a8f94fd390425c1 ;;
    linux-arm64) sha256=394ddccc2402cfa3abd97dfba56f3587781a3d6e6ce66e65ceada14beb7664b8 ;;
    *) echo "no digest for $platform" >&2; exit 1 ;;
esac

cli="$dir/tailwindcss-$version-$platform"
mkdir -p "$dir"
gh release download "v$version" --repo tailwindlabs/tailwindcss \
    --pattern "tailwindcss-$platform" --output "$cli" --clobber
echo "$sha256  $cli" | sha256sum --check
chmod +x "$cli"
