# syntax=docker/dockerfile:1

# The Tailwind CLI, put where the build looks before it downloads one. Empty here; CI replaces it
# with `--build-context tailwind=<dir>` since GitHub's release downloads can fail.
FROM scratch AS tailwind

FROM rust:1.99-bookworm AS builder

# The CLI bundles the assets the panels load at runtime. It must be the Topcoat release Cargo.lock
# records, or the bundle it writes is unreadable.
ARG TOPCOAT_VERSION=0.12.0

WORKDIR /src
RUN cargo install topcoat-cli --version "${TOPCOAT_VERSION}" --locked
COPY . .
COPY --from=tailwind / /src/target/topcoat/cache/tailwind/
RUN topcoat asset bundle --release --bin tablo-saas

# Distroless keeps the runtime to the C library the binary needs, and runs as nobody.
FROM gcr.io/distroless/cc-debian12:nonroot

COPY --from=builder /src/target/release/tablo-saas /app/tablo-saas
COPY --from=builder /src/target/release/assets /app/assets

ENV HOST=0.0.0.0 \
    PORT=3000 \
    APP_ENV=production \
    LOG_FORMAT=json

EXPOSE 3000
USER nonroot:nonroot
WORKDIR /app

ENTRYPOINT ["/app/tablo-saas"]
CMD ["serve"]
