FROM lukemathwalker/cargo-chef:latest-rust-bookworm AS chef
RUN apt install libsqlite3-dev
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder 
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin token-manager

FROM gcr.io/distroless/cc-debian12 AS runtime
COPY --from=builder /lib/x86_64-linux-gnu/libsqlite3.* /lib/x86_64-linux-gnu/
COPY --from=builder /app/target/release/token-manager /usr/local/bin/
ENTRYPOINT ["/usr/local/bin/token-manager"]
