FROM rust:1.81-bookworm as builder

WORKDIR /usr/src/app
RUN USER=root cargo new --bin emoji-matcher
WORKDIR /usr/src/app/emoji-matcher

COPY ./Cargo.toml ./Cargo.toml

RUN cargo build --release

RUN rm src/*.rs

COPY ./src ./src

RUN touch src/main.rs
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    openssl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/emoji-matcher/target/release/zako3-emoji-matcher /usr/local/bin/emoji-matcher

CMD ["emoji-matcher"]
