# Multi-stage: deps in one layer, code in another → fast rebuilds on code change
FROM rust:1-bookworm AS build
WORKDIR /app

# cache dependencies: build with stub main first
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main(){}" > src/main.rs && cargo build --release && rm -rf src

COPY . .
# SQLx needs DATABASE_URL at compile time for query macros — use offline metadata
ENV SQLX_OFFLINE=true
# BuildKit preserves source mtimes → cargo thinks our crate is unchanged and
# keeps the stub binary. Force-rebuild by nuking our crate's artifacts.
RUN rm -rf target/release/backend target/release/deps/backend* target/release/.fingerprint/backend-* \
    && cargo build --release && test -x target/release/backend

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/target/release/backend ./backend
COPY migrations ./migrations
EXPOSE 3001
CMD ["./backend"]
