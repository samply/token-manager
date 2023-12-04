# Compile Rust project
# FROM rust:latest AS builder
# WORKDIR /workdir
# COPY . ./

# RUN cargo build --release

# Create Final image
#FROM gcr.io/distroless/cc-debian12
#FROM debian:buster-slim
#FROM ubuntu
FROM debian
#COPY --from=builder /workdir/target/release/token-manager /app/
COPY ./data /app/data
COPY ./target/release/token-manager /app/
RUN chmod +x /app

RUN apt-get update && apt-get install -y sqlite3 && apt clean && rm -rf /var/lib/apt/lists/*

#RUN apt-get update && apt-get install -y openssl && rm -rf /var/lib/apt/lists/*
# # Install dependencies
# RUN apt-get update && \
#     apt-get install -y sqlite3 libsqlite3-dev && \
#     cargo install diesel_cli --no-default-features --features sqlite

CMD ["./app/token-manager"]