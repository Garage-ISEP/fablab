FROM rust:alpine AS builder

RUN apk add --no-cache musl-dev pkgconfig openssl-dev perl make

WORKDIR /app

COPY Cargo.toml Cargo.lock ./

COPY src/ ./src/
COPY migrations/ ./migrations/
COPY style/ ./style/
COPY public/ ./public/

RUN cargo build --release

FROM alpine:latest

ARG APP_UID=1001
ARG APP_GID=1001

RUN apk add --no-cache curl && \
    addgroup -S -g ${APP_GID} fablab && \
    adduser -S -G fablab -u ${APP_UID} fablab

WORKDIR /app

COPY --from=builder /app/target/release/fablab ./fablab
COPY --from=builder /app/migrations/ ./migrations/
COPY --from=builder /app/style/ ./style/
COPY --from=builder /app/public/ ./public/

RUN mkdir -p /app/data /app/data/uploads && \
    chown -R fablab:fablab /app/data && \
    chmod 700 /app/data/uploads

USER ${APP_UID}:${APP_GID}

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://127.0.0.1:3000/ || exit 1

CMD ["./fablab"]