# -*- mode: dockerfile -*-
ARG BASE_IMAGE=ekidd/rust-musl-builder:1.36.0
ARG BIN_NAME=amqprelay
FROM guangie88/muslrust-extra:stable as static_factory
# Our first FROM statement declares the build environment.
FROM ${BASE_IMAGE} AS builder

COPY --from=static_factory \
    /musl/lib/libmysqlclient.a /home/rust/src/ 

ENV MYSQLCLIENT_LIB_DIR=/home/rust/src

# Add our source code. Do not forget to tune your .dockerignore file to avoid passing heavy resources like "target" into docker context
ADD . ./

# Fix permissions on source code.
# RUN sudo chown -R rust:rust /home/rust

# Change binary name from "aeroweb" to actual name of your binary if you decide to use this file for another project :)
RUN echo "#!/bin/bash" > ./docker-entrypoint.sh && \
    echo "cargo build --release && cp /home/rust/src/target/x86_64-unknown-linux-musl/release/$BIN_NAME /home/rust/dist" >> ./docker-entrypoint.sh && \
    chmod +x docker-entrypoint.sh

# Here is place for you builds. You can make ./dist dir on you host to make mapping more clear
VOLUME [ "/home/rust/dist" ]

ENTRYPOINT [ "./docker-entrypoint.sh" ]

# Usage: 

# $ docker build . -t your-builder
# $ docker run --rm -it -v "$(pwd)/dist":/home/rust/dist your-builder /bin/bash
# Indeed your binary is ready immediatelly when container started, look inside your "$(pwd)/dist" :)