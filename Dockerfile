FROM rust:1.96-slim AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY gateway ./gateway
RUN cargo build --release --bin secure-ai-gateway

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/secure-ai-gateway /usr/local/bin/
EXPOSE 8005
CMD ["secure-ai-gateway"]
