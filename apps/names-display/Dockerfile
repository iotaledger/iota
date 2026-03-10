FROM node:22-alpine

# Install pnpm
RUN corepack enable

# Set the working directory to the monorepo root
WORKDIR /workspace

# Copy workspace configuration and root package files
COPY pnpm-workspace.yaml package.json pnpm-lock.yaml ./

# Copy all source code
COPY sdk ./sdk
COPY names-display ./names-display
COPY build-scripts ./build-scripts
COPY packages ./packages

# Install all dependencies
RUN pnpm install --frozen-lockfile

# Build the SDK dependency first
RUN cd sdk && pnpm build

# Build the names-display service
RUN cd names-display && pnpm build

# Set working directory to names-display
WORKDIR /workspace/names-display

# Expose port
EXPOSE 3000

# Set environment variables
ENV NODE_ENV=production
ENV PORT=3000

# Start the application
CMD ["pnpm", "start"]
