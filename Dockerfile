# Official rust:alpine, served from AWS ECR Public (Docker Official Images mirror)
# to avoid Docker Hub rate limits. Byte-identical to docker.io/library/rust — same
# manifest digest. Kept on alpine/musl: the binary is a static musl build into
# `FROM scratch`, and musl resolves getaddrinfo cleanly when statically linked
# (glibc-static breaks NSS-based resolution).
FROM public.ecr.aws/docker/library/rust:1-alpine3.24@sha256:f87aa870663e2b57ec8c69de82c7eedf7383bee987eef7612c0359635eaadb41 AS builder
RUN apk add --no-cache musl-dev
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY benches ./benches
RUN cargo build --release --bin dns-forwarder --target aarch64-unknown-linux-musl

FROM scratch
COPY --from=builder /build/target/aarch64-unknown-linux-musl/release/dns-forwarder /dns-forwarder
ENTRYPOINT ["/dns-forwarder"]
