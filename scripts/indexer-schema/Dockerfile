# Use the official PostgreSQL image as a base
FROM postgres:15

# Install dependencies
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    libpq-dev && \
    rm -rf /var/lib/apt/lists/*

ARG RUST_TOOLCHAIN_VERSION

# Install Rust
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain ${RUST_TOOLCHAIN_VERSION}

# Install Diesel CLI
RUN /root/.cargo/bin/cargo install diesel_cli --no-default-features --features postgres

# Add the Rust and Diesel CLI binaries to PATH
ENV PATH="/root/.cargo/bin:${PATH}"

