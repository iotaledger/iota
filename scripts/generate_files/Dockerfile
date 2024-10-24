FROM node:20-bookworm

# Accept a build argument for the Rust version
ARG RUST_VERSION=1.80.1

# Get USER_ID and GROUP_ID from build-args
ARG USER_ID=1000
ARG GROUP_ID=1000

# Check if the USER_ID or GROUP_ID need to be updated, and adjust permissions accordingly
RUN if [ $GROUP_ID -ne 1000 ]; then \
    groupmod -g $GROUP_ID node; \
fi && \
if [ $USER_ID -ne 1000 ]; then \
    usermod -u $USER_ID node; \
fi && \
if [ $USER_ID -ne 1000 ] || [ $GROUP_ID -ne 1000 ]; then \
    chown -R $USER_ID:$GROUP_ID /home/node; \
fi

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