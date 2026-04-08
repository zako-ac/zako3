FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 libopus0 && rm -rf /var/lib/apt/lists/*
COPY target/release/hq-boot /usr/local/bin/hq-boot
CMD ["hq-boot"]
