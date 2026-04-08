FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 libopus0 && rm -rf /var/lib/apt/lists/*
COPY dist/zako3-audio-engine-controller /usr/local/bin/zako3-audio-engine-controller
CMD ["zako3-audio-engine-controller"]
