FROM node:24-alpine AS base

WORKDIR /app

# Enable corepack so that pnpm version from package.json is used
RUN corepack enable

# ---------- Dependencies + Build ----------
FROM base AS builder

# Files needed for pnpm and workspaces
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml turbo.json Cargo.toml ./

COPY apps ./apps
COPY sdk ./sdk

RUN pnpm config set auto-install-peers=false \
    && pnpm install --frozen-lockfile

RUN pnpm apps-backend build

# ---------- Runtime image ----------
FROM node:24-alpine AS runner

WORKDIR /app
RUN corepack enable
ENV NODE_ENV=production

COPY --from=builder /app ./

WORKDIR /app/apps/apps-backend

EXPOSE 3003

CMD ["node", "dist/apps-backend/src/main.js"]
