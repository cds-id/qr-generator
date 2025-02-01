FROM rust:bookworm as builder

WORKDIR /usr/src/app
COPY . .

# Build for release
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install required runtime dependencies
RUN apt-get update && apt-get install -y \
    libssl-dev \
    openssl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /usr/src/app/target/release/qr-generator /usr/local/bin/

# Create a non-root user
RUN useradd -m -U -s /bin/false qrservice

# Switch to non-root user
USER qrservice

# Expose the port
EXPOSE 8080

# Run the binary
CMD ["qr-generator"]
