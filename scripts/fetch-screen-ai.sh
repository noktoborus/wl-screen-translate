#!/bin/sh
# Downloads the Chrome Screen AI library from Google's package server into
# $DATA/screen-ai, or copies it from a Chrome profile with --from-chrome.
set -eu
. "$(dirname "$0")/common.sh"

TARGET=$DATA/screen-ai
CHROME_COMPONENT=${CHROME_COMPONENT:-$HOME/.config/google-chrome/screen_ai}
API=https://chrome-infra-packages.appspot.com/prpc/cipd.Repository
PACKAGE=${PACKAGE:-chromium/third_party/screen-ai/linux}

if [ "${1:-}" = --from-chrome ]; then
    # Chrome keeps one directory per version; the newest is taken.
    source=$(ls -d "$CHROME_COMPONENT"/*/ 2>/dev/null | sort -V | tail -n 1)
    [ -n "$source" ] || die "no Screen AI component in $CHROME_COMPONENT"
    mkdir -p "$TARGET"
    cp -r "$source". "$TARGET/"
    echo "copied $source to $TARGET"
    exit 0
fi

need curl jq unzip

# The server answers JSON behind a ")]}'" line.
prpc() {
    curl -fsS -H 'Content-Type: application/json' -H 'Accept: application/json' \
        -d "$2" "$API/$1" | sed 1d
}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

instance=$(prpc ResolveVersion "{\"package\":\"$PACKAGE\",\"version\":\"latest\"}" | jq -c .instance)
url=$(prpc GetInstanceURL "{\"package\":\"$PACKAGE\",\"instance\":$instance}" | jq -r .signedUrl)
echo "downloading $PACKAGE"
curl -fL --progress-bar "$url" -o "$work/screen-ai.zip"
unzip -q "$work/screen-ai.zip" 'resources/*' -d "$work"
mkdir -p "$TARGET"
cp -r "$work/resources/." "$TARGET/"
echo "Screen AI is in $TARGET"
