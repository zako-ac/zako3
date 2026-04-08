FROM node:22-alpine AS base
RUN npm install -g pnpm
WORKDIR /app

FROM base AS deps
COPY pnpm-workspace.yaml pnpm-lock.yaml package.json ./
COPY packages/zako3-data/package.json ./packages/zako3-data/
COPY web/package.json ./web/
RUN pnpm install --frozen-lockfile

FROM deps AS builder
COPY packages/zako3-data/ ./packages/zako3-data/
COPY web/ ./web/
ARG VITE_API_BASE_URL=http://localhost:8080/api/v1
ARG VITE_WS_BASE_URL=ws://localhost:8080
ARG VITE_GRAFANA_URL=http://localhost:3000
RUN pnpm --filter @zako-ac/web build

FROM nginx:alpine AS runtime
COPY web/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/web/dist /usr/share/nginx/html
EXPOSE 80
