#!/bin/bash

set -eux

export CRATE_NAME=$1
name="$CRATE_NAME-$TRAVIS_TAG-$TARGET"
mkdir -p "$name"
# cp "target/$TARGET/release/$CRATE_NAME" "$name/"
cp "target/$TARGET/release/*" "$name/"
# cp cargo-crev/README.md LICENSE* "$name/"
tar czvf "$name.tar.gz" "$name"

# Get the sha-256 checksum w/o filename and newline
echo -n $(shasum -ba 256 "$name.tar.gz" | cut -d " " -f 1) > "$name.tar.gz.sha256"