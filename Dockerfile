FROM rust:latest AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./

RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    echo "" > src/lib.rs && \
    cargo build --release 2>/dev/null || true && \
    rm -rf src

COPY src/ ./src/
COPY migrations/ ./migrations/

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/fablab ./fablab
COPY migrations/ ./migrations/
COPY style/ ./style/
COPY public/ ./public/

RUN mkdir -p /app/data /app/data/uploads && \
    useradd --system --no-create-home fablab && \
    chown -R fablab:fablab /app/data && \
    chmod 700 /app/data/uploads

USER fablab

EXPOSE 3000

CMD ["./fablab"]
