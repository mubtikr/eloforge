# syntax=docker/dockerfile:1.7

FROM node:24-alpine AS build
WORKDIR /app
RUN corepack enable
COPY package.json pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY . .
RUN pnpm build

FROM caddy:2-alpine AS runtime
RUN apk add --no-cache su-exec \
    && addgroup -S -g 10002 eloforge \
    && adduser -S -D -H -u 10002 -G eloforge eloforge \
    && mkdir -p /data /config /srv \
    && chown -R eloforge:eloforge /data /config /srv
COPY Caddyfile /etc/caddy/Caddyfile
COPY docker-entrypoint.sh /usr/local/bin/eloforge-web-entrypoint
RUN chmod +x /usr/local/bin/eloforge-web-entrypoint
COPY --from=build --chown=10002:10002 /app/dist /srv
ENV XDG_CONFIG_HOME=/config \
    XDG_DATA_HOME=/data
EXPOSE 8080 8443
ENTRYPOINT ["/usr/local/bin/eloforge-web-entrypoint"]
CMD ["caddy", "run", "--config", "/etc/caddy/Caddyfile", "--adapter", "caddyfile"]
