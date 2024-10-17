FROM node:20-bookworm

# Install pnpm
RUN npm install -g pnpm

USER node
WORKDIR /home/node

# Set the safe.directory to /app
RUN git config --global --add safe.directory /app

RUN mkdir -p /home/node/app

WORKDIR /home/node/app