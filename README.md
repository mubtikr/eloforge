# EloForge

A self-hosted chess study room. EloForge imports your real Chess.com games, evaluates every move with Stockfish 17 through a Rust analyzer, turns your own mistakes into spaced-repetition puzzles and endgame sparring sessions, and keeps a deterministic coaching record that never lets a language model invent chess facts.

```
Browser (React 19 + chessground)
        │  HTTPS + SSE
        ▼
Caddy ──▶ Rust API (Axum + Tokio)
            ├── Chess.com serial sync (ETag, 429 backoff)
            ├── Stockfish 17 worker pool (bounded)
            ├── shakmaty + ECO + mistake tagger
            ├── FSRS puzzles + endgame sparring
            └── OpenCode Go coach (DeepSeek V4.1 Flash default)
        │                    │
        ▼                    ▼
PostgreSQL 17 + pgvector   Valkey 8 (queues, Pub/Sub, SSE fan-out)
```

## Quick start (Docker)

```bash
cp .env.example .env          # optional for local; compose has safe dev defaults
docker compose up --build -d
curl http://localhost:8080/api/healthz
open http://localhost:3000
```

The health endpoint reports PostgreSQL, Valkey and the Stockfish UCI pool separately. First build compiles Stockfish 17 and the Rust API for the host architecture; later builds are cached.

## Local development

```bash
# services only
docker compose up -d postgres valkey

cp .env.example .env
# set DATABASE_URL=postgres://eloforge:eloforge@localhost:5432/eloforge
cargo run -p eloforge-api          # API on :8080

cd frontend
pnpm install
pnpm dev                           # Vite on :5173, proxies /api and /metrics
```

`STOCKFISH_PATH=stockfish` must resolve to a UCI engine; `stockfish` on PATH is fine outside Docker.

## Verification gates

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test

cd frontend
pnpm typecheck
pnpm lint
pnpm build
npx impeccable detect src/

docker compose config
docker compose up --build -d
```

`make check` runs the full local gate set.

## Configuration

All configuration is environment-driven; see `.env.example`. The important ones:

| Variable | Default | Notes |
| --- | --- | --- |
| `JWT_SECRET` | — | Required, ≥ 32 chars. Generate with `openssl rand -hex 32`. |
| `OPENCODE_GO_MODEL` | `deepseek-v4.1-flash` | EloForge's default coach model. |
| `OPENCODE_GO_PROTOCOL` | `chat` | `chat`, `messages`, or `responses` to match the model's upstream protocol. |
| `OPENCODE_API_KEY` | empty | Server-side only. Without it, coaching degrades to deterministic engine notes. |
| `OPENCODE_GO_BASE_URL` | `https://opencode.ai/zen/go/v1` | Must include `/v1`. |
| `STOCKFISH_WORKERS` | CPU count − 1 | Bounded pool size. |
| `CHESSCOM_USER_AGENT` | `EloForge/1.0 (contact: …)` | Required polite identification. |
| `COOKIE_SECURE` | `true` | Set `false` only for plain-HTTP local development. |

## Project layout

```
backend/        Rust API, analyzer, engine pool, prompts, SQL migrations
frontend/       Vite + React 19 + Tailwind v4 + chessground
docs/           architecture, PRD, research, deployment
scripts/        backup + operational helpers
```

## Guardrails

- Stockfish 17 and `shakmaty` are the sole source of truth for legality, evaluations, tactical tags, ECO codes and phase detection.
- OpenCode Go is called only from the backend, always with `x-opencode-session` and `User-Agent: EloForge/1.0`.
- Chess.com PubAPI requests are strictly serial with ETag caching and respectful 429 backoff.
- Containers run as a non-root user; every user-owned query is scoped by `auth.user_id`.

## Disclaimer

Independent training tool. Not affiliated with or endorsed by Chess.com. Chess.com board colours, piece sprites, sounds, classification glyphs and feature names are not used.

## License

MIT
