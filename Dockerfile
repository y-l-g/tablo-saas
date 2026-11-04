# syntax=docker/dockerfile:1

FROM rust:1.99-bookworm AS builder

# The CLI bundles the assets the panels load at runtime. It must come from the same Topcoat commit
# as the `[patch.crates-io]` section of Cargo.toml, or the bundle it writes is unreadable.
ARG TOPCOAT_REV=d84362b72bc1034a8342e00c1cdf1a36d68dd9cf

WORKDIR /src
RUN cargo install --git https://github.com/tokio-rs/topcoat --rev "${TOPCOAT_REV}" topcoat-cli --locked
COPY . .
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
