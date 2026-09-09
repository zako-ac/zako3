FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY dist/zako3-ae-proxy /usr/local/bin/zako3-ae-proxy
CMD ["zako3-ae-proxy"]
