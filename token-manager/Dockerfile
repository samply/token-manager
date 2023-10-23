# Compile Rust project
FROM rust:latest AS builder
WORKDIR /workdir
COPY . ./
RUN cargo build --release

# Create Final image
FROM gcr.io/distroless/cc-debian12
COPY --from=builder /workdir/target/release/token-manager /usr/local/bin/
ENTRYPOINT ["/usr/local/bin/token-manager"]
