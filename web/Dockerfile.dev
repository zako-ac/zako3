FROM node:22-alpine AS builder
RUN npm install -g pnpm
WORKDIR /app
COPY pnpm-workspace.yaml ./
COPY pnpm-lock.yaml ./
COPY packages/zako3-data/ ./packages/zako3-data/
COPY web/ ./web/
ARG VITE_API_BASE_URL=/api/v1
ARG VITE_WS_BASE_URL=ws://localhost
ARG VITE_GRAFANA_URL=http://localhost:3000
RUN pnpm install --filter @zako-ac/web...
WORKDIR /app/web
RUN pnpm build

FROM nginx:alpine AS runtime
COPY web/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/web/dist /usr/share/nginx/html
EXPOSE 80
