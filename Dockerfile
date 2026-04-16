FROM rust:latest AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./

# Dummy build pour cacher les dépendances (lib + bin)
RUN mkdir -p src && \
    echo 'fn main() {}' > src/main.rs && \
    echo '' > src/lib.rs && \
    cargo build --release 2>/dev/null || true && \
    rm -rf src

COPY src/ ./src/
COPY migrations/ ./migrations/
COPY style/ ./style/
COPY public/ ./public/

RUN cargo build --release

RUN mkdir -p target/site && \
    cp -r public/* target/site/ && \
    mkdir -p target/site/_style && \
    cp style/main.css target/site/main.css



FROM debian:bookworm-slim

ARG APP_UID=1001
ARG APP_GID=1001

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libssl3 curl && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd --system --gid ${APP_GID} fablab && \
    useradd --system --no-create-home --uid ${APP_UID} --gid ${APP_GID} fablab

WORKDIR /app

COPY --from=builder /app/target/release/fablab ./fablab
COPY --from=builder /app/target/site ./target/site
COPY migrations/ ./migrations/

RUN mkdir -p /app/data /app/data/uploads && \
    chown -R fablab:fablab /app/data && \
    chmod 700 /app/data/uploads

USER ${APP_UID}:${APP_GID}

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:3000/ || exit 1

CMD ["./fablab"]