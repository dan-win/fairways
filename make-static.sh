#!/bin/bash
docker build . -t fairways-builder
docker run --rm -it -v "$(pwd)/dist":/home/rust/dist fairways-builder /bin/bash