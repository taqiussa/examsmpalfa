# =========================
# FE Builder (Vite + Tailwind)
# =========================
FROM node:20-bookworm-slim AS fe_builder

WORKDIR /app/fe

COPY fe/package.json fe/package-lock.json ./
RUN npm ci --no-audit --no-fund --fetch-retries=5 --fetch-retry-factor=2 --fetch-retry-mintimeout=20000 --fetch-retry-maxtimeout=120000

COPY fe/ ./
COPY templates/ ../templates/

RUN npm run build


# =========================
# Rust Builder
# =========================
FROM rust:1.92.0-bookworm AS rust_builder

WORKDIR /app

ARG SQLX_OFFLINE=true
ENV SQLX_OFFLINE=${SQLX_OFFLINE}

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    default-libmysqlclient-dev \
    libssl-dev \
    ca-certificates \
 && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY .sqlx/ ./.sqlx/
RUN mkdir src && printf "fn main() {}\n" > src/main.rs
RUN cargo build --release --locked
RUN rm -rf src

COPY src/ ./src/
COPY templates/ ./templates/
COPY .sqlx/ ./.sqlx/

RUN rm -f /app/target/release/sikadusmpmifda \
    && find src -type f -exec touch {} + \
    && cargo build --release --locked \
    && strip /app/target/release/sikadusmpmifda


# =========================
# Runtime
# =========================
FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    libmariadb3 \
 && rm -rf /var/lib/apt/lists/*

COPY --from=rust_builder /app/target/release/sikadusmpmifda /app/app
COPY --from=rust_builder /app/templates ./templates
COPY --from=fe_builder /app/static ./static

ENV APP_ENV=production
ENV APP_PORT=3000

EXPOSE 3000
CMD ["/app/app"]
