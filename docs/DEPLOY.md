# DEPLOY.md — self-hosted EloForge

EloForge is designed to run as a single Docker Compose stack on a small VPS. A 4 vCPU / 4 GB RAM machine handles a personal account comfortably; give Stockfish at least two cores and keep `STOCKFISH_WORKERS` at or below `nproc - 1`.

## 1. Prerequisites

- Docker Engine 26+ with Compose v2, on Linux amd64 or arm64.
- A DNS A/AAAA record pointing at the VPS if you want automatic HTTPS.
- Ports 80 and 443 open (or an external reverse proxy in front of the stack).
- A long random JWT secret: `openssl rand -hex 32`.

## 2. First deployment

```bash
git clone https://github.com/<you>/eloforge.git
cd eloforge
cp .env.example .env
```

Edit `.env`:

```dotenv
JWT_SECRET=<openssl rand -hex 32>
COOKIE_SECURE=true
FRONTEND_ORIGIN=https://chess.example.com
SITE_ADDRESS=chess.example.com
WEB_PORT=80
WEB_TLS_PORT=443

# Required for deterministic-only mode; set the key to enable the AI coach.
OPENCODE_API_KEY=
OPENCODE_GO_MODEL=deepseek-v4.1-flash
OPENCODE_GO_PROTOCOL=chat
AI_DAILY_TOKEN_BUDGET=250000

CHESSCOM_USER_AGENT=EloForge/1.0 (contact: you@example.com)
STOCKFISH_WORKERS=3
STOCKFISH_HASH_MB=128
ANALYSIS_MOVETIME_FULL_MS=200
ANALYSIS_MOVETIME_CRITICAL_MS=800
```

`SITE_ADDRESS=chess.example.com` makes Caddy obtain and renew a certificate automatically. Caddy stores it in the `caddy_data` volume. For a local-only deployment leave `SITE_ADDRESS=:80`, `WEB_PORT=3000`, `COOKIE_SECURE=false`.

Start the stack:

```bash
docker compose up -d --build
curl -fsS https://chess.example.com/api/healthz
```

The health response must report `database`, `valkey` and `stockfish` as `ok`.

## 3. Post-deploy checks

- Open `https://chess.example.com`, connect a Chess.com account with the profile nonce challenge, then press **Sync Chess.com**.
- Confirm `/metrics` is reachable from the monitoring network only. If Caddy is public, restrict `/metrics` at the firewall or reverse proxy; it has no authentication by design.
- Check that `games.analysis_status` moves from `pending` to `queued`, `analyzing`, `done`.
- Run one reflection and confirm the `ai_status` is `ok` with a key, or `unavailable` with deterministic notes when no key is configured.
- Complete one puzzle and one sparring drill.

## 4. Backups

```bash
./scripts/backup.sh
```

Backups land in `backups/eloforge-YYYYmmdd-HHMMSS.dump` and older files are pruned after `BACKUP_RETENTION_DAYS` (default 14). Restore with:

```bash
./scripts/restore.sh backups/eloforge-YYYYmmdd-HHMMSS.dump
```

The restore script stops the API, runs `pg_restore --clean`, then restarts it. Put `./scripts/backup.sh` on cron and copy backups off the VPS:

```cron
15 3 * * * cd /srv/eloforge && ./scripts/backup.sh >> /var/log/eloforge-backup.log 2>&1
```

## 5. Upgrades

```bash
git pull
docker compose up -d --build
curl -fsS https://chess.example.com/api/healthz
```

SQLx migrations run automatically at API start. Back up before every upgrade; migrations are additive but a bad release should still be recoverable.

## 6. Monitoring

Prometheus can scrape:

```
https://chess.example.com/metrics
```

Important series:

- `eloforge_stockfish_queue_depth` — queued engine queries.
- `eloforge_analysis_latency_seconds` — full-game analysis latency.
- `eloforge_analysis_failures_total` — failed analysis jobs.
- `eloforge_ai_requests_total`, `eloforge_ai_errors_total`, `eloforge_ai_tokens_total`.
- `eloforge_sync_games_total`, `eloforge_sync_errors_total`.
- `eloforge_sse_connections`.

Set a log driver limit in production so JSON logs do not fill the disk:

```yaml
logging:
  driver: json-file
  options:
    max-size: "20m"
    max-file: "5"
```

Add this to the `api` and `web` services or configure it in Coolify.

## 7. Chess.com compliance

- Keep the `CHESSCOM_USER_AGENT` contact address live. Chess.com may block requests without a contactable agent.
- Sync is serial and ETag-cached. Do not add a parallel crawler in front of it.
- Do not expose the raw PubAPI responses as a public mirror. The app stores game analysis for the authenticated owner only.

## 8. OpenCode Go

AI calls are backend-only. The server sends:

- `Authorization: Bearer <OPENCODE_API_KEY>`
- `User-Agent: EloForge/1.0`
- `x-opencode-session: <stable UUID>`
- protocol-specific request to `https://opencode.ai/zen/go/v1`

The default model is `deepseek-v4.1-flash` over the OpenAI-compatible `chat/completions` protocol. If a model changes protocol, set `OPENCODE_GO_PROTOCOL` to `chat`, `messages` or `responses`; do not change the code. Without a key, reflections and weekly briefings use deterministic engine facts so the product remains usable.

## 9. Rollback

```bash
git checkout <previous-tag>
docker compose up -d --build
./scripts/restore.sh backups/<pre-upgrade>.dump
```

Keep the previous image tags and a database dump until the smoke test passes.
