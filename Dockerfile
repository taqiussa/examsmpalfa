
# =====================================
# Frontend Builder
# =====================================
FROM node:20-bookworm-slim AS fe_builder

WORKDIR /app/fe

COPY fe/package.json fe/package-lock.json ./

RUN npm ci --no-audit --no-fund

COPY fe/ ./
COPY templates/ ../templates/

RUN npm run build

# =====================================
# Rust Builder
# =====================================
FROM rust:1.94.0-bookworm AS rust_builder

WORKDIR /app

ENV SQLX_OFFLINE=true

RUN apt-get update && apt-get install -y \
    --no-install-recommends \
    pkg-config \
    default-libmysqlclient-dev \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Dependensi SQLx CLI untuk MariaDB
RUN cargo install sqlx-cli \
    --no-default-features \
    --features rustls,mysql \
    --locked

# Dependency caching
COPY Cargo.toml Cargo.lock ./
COPY .sqlx/ ./.sqlx/

RUN mkdir src \
    && printf "fn main() {}\n" > src/main.rs

RUN cargo build --release --locked

RUN rm -rf src

# Source utama
COPY src/ ./src/
COPY templates/ ./templates/
COPY migrations/ ./migrations/

RUN rm -f /app/target/release/exam \
    && find src -type f -exec touch {} + \
    && cargo build --release --locked \
    && strip /app/target/release/exam

# =====================================
# Production Runtime
# =====================================
FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update && apt-get install -y \
    --no-install-recommends \
    ca-certificates \
    libssl3 \
    libmariadb3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=rust_builder \
    /app/target/release/exam /app/exam

COPY --from=rust_builder \
    /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx

COPY --from=rust_builder \
    /app/templates ./templates

COPY --from=rust_builder \
    /app/migrations ./migrations

COPY --from=fe_builder \
    /app/static ./static

ENV APP_ENV=production \
    APP_PORT=3000

EXPOSE 3000

CMD ["/app/exam"]
