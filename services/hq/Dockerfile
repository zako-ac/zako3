FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 libopus0 && rm -rf /var/lib/apt/lists/*
COPY dist/hq-boot /usr/local/bin/hq-boot
CMD ["hq-boot"]
