FROM node:20-bookworm

# Accept a build argument for the Rust version
ARG RUST_VERSION

# Install pnpm
RUN npm install -g pnpm

USER node
WORKDIR /home/node

# Set the safe.directory to /app
RUN git config --global --add safe.directory /app

# Install the specific Rust version
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --default-toolchain ${RUST_VERSION} -y

# Add .cargo/bin to PATH
ENV PATH="/home/node/.cargo/bin:${PATH}"

RUN mkdir -p /home/node/app

WORKDIR /home/node/app