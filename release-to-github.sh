#!/usr/bin/env bash

set -ex

usage() {
    echo "$(basename "$0") <version> <gh-username> <gh-token>"
    exit 1
}

if [ $# -lt 3 ]; then
    usage
fi

if ! which jq; then
  echo 'you need jq -- brew install jq or http://stedolan.github.io/jq/'
  exit 1
fi

VERSION="$1"
shift
GITHUB_USERNAME="$1"
shift
GITHUB_TOKEN="$1"
DIR="tmp/batterybar-$VERSION"
TAR="batterybar-$VERSION.tar.gz"

cleanup () {
    rm -rf "$DIR"
    rm "$(dirname "$DIR")/$TAR"
}

trap cleanup ERR EXIT

BIN="$( ./build.sh | tail -n1 )"
PLIST=com.github.blandinw.batterybar.plist

(
    # prepare tmp dir for .tar.gz
    mkdir -p "$DIR"
    cp "$BIN" "$DIR"
    cp "$PLIST" "$DIR"

    # prepare install command
    cp install.sh "$DIR/.install-helper.sh"
    cat > "$DIR/install-batterybar.command" <<EOF
#!/usr/bin/env bash
set -ex
PLIST="$PLIST"
EOF
    cat >> "$DIR/install-batterybar.command" <<'EOF'
cd "$(dirname $0)"
./.install-helper.sh batterybar "$PLIST"
echo "Installation successful!"
EOF
    chmod +x "$DIR/install-batterybar.command"

    # prepare uninstall command
    cp uninstall.sh "$DIR/.uninstall-helper.sh"
    cat > "$DIR/uninstall-batterybar.command" <<EOF
#!/usr/bin/env bash
set -ex
PLIST="$PLIST"
EOF
    cat >> "$DIR/uninstall-batterybar.command" <<'EOF'
cd "$(dirname $0)"
./.uninstall-helper.sh batterybar "$PLIST"
echo "Uninstallation successful!"
EOF
    chmod +x "$DIR/uninstall-batterybar.command"

    # zip it
    cd "$(dirname "$DIR")"
    tar czvf "$TAR" "$(basename "$DIR")"

    # create release
    CURL_OUT="$(
        curl https://api.github.com/repos/blandinw/batterybar/releases \
        -XPOST \
        -s -u "$GITHUB_USERNAME:$GITHUB_TOKEN" \
        -d '{
          "tag_name": "'"$VERSION"'",
          "name": "'"$VERSION"'"
        }'
    )"
    RELEASE_ID="$(echo "$CURL_OUT" | jq --raw-output '.id')"

    # upload archive
    curl "https://uploads.github.com/repos/blandinw/batterybar/releases/$RELEASE_ID/assets?name=$TAR" \
        -XPOST \
        -s -u "$GITHUB_USERNAME:$GITHUB_TOKEN" \
        -H 'Content-Type: application/gzip' \
        --data-binary "@$TAR"
)
