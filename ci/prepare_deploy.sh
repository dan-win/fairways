#!/bin/bash

set -eux
BIN_NAME=$1
if [[ "$TARGET" =~ "windows" ]]; then
    extension=".exe"
else    
    extension=
fi
# export BIN_NAME=$1
name="$BIN_NAME-$TRAVIS_TAG-$TARGET"
mkdir -p "$name"
echo ===========
echo $(pwd)
tree target
echo ===========

cp "target/$TARGET/release/$BIN_NAME$extension" "$name/"
# cp "target/$TARGET/release/*" "$name/"
# cp cargo-crev/README.md LICENSE* "$name/"
tar czvf "$name.tar.gz" "$name"

# Get the sha-256 checksum w/o filename and newline
echo -n $(shasum -ba 256 "$name.tar.gz" | cut -d " " -f 1) > "$name.tar.gz.sha256"