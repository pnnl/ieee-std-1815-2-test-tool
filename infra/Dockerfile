FROM rust:1.96.0-bookworm AS builder

WORKDIR /src
COPY . .
RUN cargo build --release -p reference-outstation

FROM debian:bookworm-slim

ARG GIT_COMMIT=unknown
LABEL org.opencontainers.image.revision="${GIT_COMMIT}"

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /src/target/release/reference-outstation /usr/local/bin/reference-outstation
COPY data/profiles/full.json /app/data/profiles/full.json

CMD ["reference-outstation", "--local", "0.0.0.0:20000", "--profile", "/app/data/profiles/full.json"]
