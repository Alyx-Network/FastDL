FROM rust:1-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/fastdl /usr/local/bin/fastdl
COPY config.yaml ./config.yaml
RUN mkdir -p /app/storage
ENV PORT=3000
EXPOSE 3000
CMD ["fastdl"]
