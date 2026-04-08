FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 libopus0 && rm -rf /var/lib/apt/lists/*
COPY dist/zako3-taphub-core /usr/local/bin/zako3-taphub-core
CMD ["zako3-taphub-core"]
