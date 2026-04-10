FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY dist/traffic-light /usr/local/bin/traffic-light
CMD ["traffic-light"]
