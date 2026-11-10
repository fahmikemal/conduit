# Release build for the production host: Ubuntu 22.04, glibc 2.35. The binary links glibc dynamically
# and everything else (OpenSSL, zlib-ng, mimalloc) statically. By default it is a
# profile-guided build (scripts/pgo.sh): an instrumented build serves a training workload
# inside the build container first. PGO=0 builds without it.
#   docker buildx build --output type=local,dest=dist .
#   docker buildx build --build-arg PGO=0 --output type=local,dest=dist .
FROM ubuntu:22.04 AS build
ARG RUST_VERSION=1.99.1
RUN apt-get update \
 && apt-get install -y --no-install-recommends build-essential cmake perl curl ca-certificates \
    openssl patch \
 && rm -rf /var/lib/apt/lists/*
RUN curl -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain "$RUST_VERSION" \
 && /root/.cargo/bin/rustup component add llvm-tools
ENV PATH=/root/.cargo/bin:$PATH
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY scripts/pgo.sh scripts/pgo-train.sh scripts/prepare-vendor.sh scripts/
COPY src src
# Pingora with conduit's patches, wired in through [patch.crates-io] in Cargo.toml.
COPY vendor vendor
RUN bash scripts/prepare-vendor.sh
ARG PGO=1
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/src/target \
    if [ "$PGO" = 1 ]; then \
      scripts/pgo.sh --out /conduit; \
    else \
      cargo build --release --locked && cp target/release/conduit /conduit; \
    fi \
 && /conduit --version

FROM scratch
COPY --from=build /conduit /conduit
