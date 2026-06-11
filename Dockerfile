FROM rust:1-bookworm AS builder
WORKDIR /app

# hadolint ignore=DL3008
RUN apt-get update \
    && apt-get install -y --no-install-recommends protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock* ./
COPY src ./src

RUN cargo build --release -p duihua-gateway

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=builder /app/target/release/duihua-gateway /duihua-gateway

EXPOSE 8080
ENTRYPOINT ["/duihua-gateway"]