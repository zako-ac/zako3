FROM rust:1.88-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN apt-get update && apt-get install -y cmake libopus-dev pkg-config && rm -rf /var/lib/apt/lists/*
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin zako3-audio-engine-controller

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y ca-certificates libssl3 libopus0 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/zako3-audio-engine-controller /usr/local/bin/zako3-audio-engine-controller
CMD ["zako3-audio-engine-controller"]
