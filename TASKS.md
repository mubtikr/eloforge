# TASKS.md — Agent Execution Backlog

## Phase 0: Repository, Docker & Design Setup
- [ ] **T0.1** Initialize `backend/` Rust crate (`axum`, `tokio`, `sqlx`, `shakmaty`, `pgn-reader`, `reqwest`, `tower-http`, `tracing`, `serde`, `uuid`, `thiserror`) and `frontend/` Vite + React 19 + TypeScript + Tailwind v4 app.
- [ ] **T0.2** Create `backend/Dockerfile` (multi-stage build installing official Stockfish 17 binary + non-root user) and `frontend/Dockerfile` (Vite build + Caddy static/reverse-proxy config).
- [ ] **T0.3** Create `docker-compose.yml` (`postgres: pgvector/pgvector:pg17`, `valkey:8-alpine`, `api`, `web`) + `.env.example` documenting all required vars (`OPENCODE_API_KEY`, `OPENCODE_GO_BASE_URL`, `OPENCODE_GO_MODEL`, `OPENCODE_GO_PROTOCOL`, `CHESSCOM_USER_AGENT`, `JWT_SECRET`, `STOCKFISH_WORKERS`).
- [ ] **T0.4** Install Impeccable skill (`npx impeccable install`), run `/impeccable init` (`PRODUCT.md`) and `/impeccable document` (`DESIGN.md`) defining EloForge's tactile editorial chess design system.
- [ ] **T0.5** Implement `GET /api/healthz` verifying PostgreSQL, Valkey, and Stockfish binary responsiveness.

## Phase 1: Database, Chess.com Auth & Serial Sync
- [ ] **T1.1** Write initial SQLx migrations (`0001_init.sql`) creating `users`, `auth_challenges`, `games`, `game_moves`, `move_reflections`, `puzzles`, `coaching_reports`, `milestones_and_goals`, and `sync_cursors`.
- [ ] **T1.2** Implement `backend/src/auth/`: `AuthProvider` trait, Chess.com Bio/Location Challenge flow (`POST /api/auth/chesscom/start`, `POST /api/auth/chesscom/verify`), `GET /api/auth/me`, `POST /api/auth/logout`, HttpOnly JWT cookie middleware, and `ChessComOAuthProvider` scaffold.
- [ ] **T1.3** Implement `backend/src/chesscom/client.rs`: strict serial-queue HTTP client (concurrency = 1) with mandatory `User-Agent`, `ETag`/`If-None-Match` caching, and `429` exponential backoff.
- [ ] **T1.4** Implement `backend/src/chess/pgn_importer.rs` using `pgn-reader` to parse games, extract `[%clk]` timestamps into `clock_remaining_sec` and `time_spent_sec`, cluster sessions (<30 min gap), and upsert idempotently into `games` + `game_moves`.
- [ ] **T1.5** Expose `POST /api/sync/trigger` and `GET /api/events/stream` (SSE) streaming live sync & analysis progress events via Valkey Pub/Sub.

## Phase 2: Stockfish Worker Pool & Deterministic Analysis
- [ ] **T2.1** Build `backend/src/engine/stockfish_pool.rs`: persistent UCI process pool bounded by `STOCKFISH_WORKERS`, configurable `Hash`, per-query timeout, and automatic crash respawn.
- [ ] **T2.2** Bundle Lichess ECO opening TSV (`backend/assets/eco.tsv`) and implement `backend/src/chess/openings.rs` to tag ECO code, opening name, last book ply, and opening-principle violations (uncastled past move 12, repeated piece moves, early queen sorties).
- [ ] **T2.3** Build `backend/src/analyzer/game_analyzer.rs`: 2-pass Stockfish analysis (depth 14 full game -> depth 20 MultiPV=3 on turning points where `EPL >= 0.10`), Win% curve, Accuracy % (overall + per phase), and max advantage tracking.
- [ ] **T2.4** Build `backend/src/analyzer/mistake_tagger.rs` using `shakmaty` + Stockfish PV to deterministically assign `HANGING_PIECE`, `MISSED_FREE_PIECE`, `MISSED_MATE_1_2`, `ALLOWED_MATE`, `TACTICAL_BLINDNESS`, `WINNING_GAME_THROWN`, `STALEMATE_THROW`, `ENDGAME_TECHNIQUE_FAIL`, `IMPULSIVE_BLUNDER`, and `TIME_SCRAMBLE_BLUNDER`.
- [ ] **T2.5** Build `backend/src/analyzer/focus_tilt.rs` computing Impulsivity Index, Post-Blunder Cascade rate, Session Tilt drop-off, and Hour-of-Day accuracy heatmap.
- [ ] **T2.6** Wire background analysis worker consuming Valkey job queue and updating `games.analysis_status` + emitting SSE progress events.
- [ ] **T2.7** Write unit & integration tests validating move classification, blunder tagging, and clock parsing against sample 500-Elo Rapid PGNs.

## Phase 3: OpenCode Go AI Coach & Reflection Memory
- [ ] **T3.1** Build `backend/src/ai/provider.rs` (`LlmProvider` trait) and `backend/src/ai/opencode_go.rs` supporting `https://opencode.ai/zen/go/v1`, custom `User-Agent`, mandatory `x-opencode-session` UUID header, protocol routing (`chat` / `messages` / `responses`), UTF-8 SSE streaming, and 502/503/529 retries.
- [ ] **T3.2** Create prompt templates under `backend/prompts/`: `match_coach.md`, `reflection_coach.md`, `weekly_briefing.md`, and `curriculum_next.md` (with strict Elo-band tone adaptation and anti-hallucination constraints).
- [ ] **T3.3** Implement `POST /api/games/{id}/moves/{move_id}/reflect` saving user explanation, retrieving past reflections with matching mistake tags, calling OpenCode Go with the game's stable `x-opencode-session` UUID, and storing `ai_feedback` + `next_time_rule`.
- [ ] **T3.4** Implement `POST /api/reports/weekly/generate` and `GET /api/reports/*` endpoints covering all 8 reports in `PRD.md`.

## Phase 4: Mistake Puzzles, Endgame Sparring & Road-to-GM Engine
- [ ] **T4.1** Build `backend/src/training/puzzle_extractor.rs`: auto-create `blunder_fix` puzzles from every user blunder (`EPL >= 0.15`) and `endgame_conversion` drills from every `WINNING_GAME_THROWN` / `ENDGAME_TECHNIQUE_FAIL`.
- [ ] **T4.2** Implement FSRS spaced-repetition scheduler (`GET /api/puzzles/due`, `POST /api/puzzles/{id}/attempt`).
- [ ] **T4.3** Implement Endgame Sparring API (`POST /api/sparring/move`): accepts user move on a failed endgame FEN, validates via `shakmaty`, returns rating-calibrated Stockfish reply (`UCI_LimitStrength` + `UCI_Elo`), and marks drill mastered when checkmate/decisive promotion is achieved.
- [ ] **T4.4** Implement Road-to-GM Milestone Engine (`backend/src/goals/ladder.rs`): seed Tiers 1–8 (500 → 650 → 800 → 1000 → 1200 → 1500 → 1800 → 2200 → 2500), auto-recompute progress after every analyzed game, and expose `GET /api/roadmap`.
- [ ] **T4.5** Implement Goal & Reminder Worker (`backend/src/notifications/`): cron runner sending daily study nudges, streak alerts, and tilt warnings via Telegram Bot Webhook, SMTP email, or Web Push.

## Phase 5: Frontend Application & Impeccable Design Pass
- [ ] **T5.1** Build Auth / Onboarding view: "Connect Chess.com" flow with verification code copy button, live check, and non-affiliation footer.
- [ ] **T5.2** Build Main Coach Dashboard: Current Elo + Road-to-1000 progress bar, Top 3 Rating Leaks, Today's Due Blunder Puzzles count, Weekly AI Coach Briefing, and live Sync button.
- [ ] **T5.3** Build Match Lab (`/games/:id`): 60fps `chessground` board with best-move vs. blunder arrows, synced Eval + Clock timeline chart, phase accuracy breakdown, deterministic mistake badges, and interactive **"Why did I play this?"** AI Coach Reflection drawer (streaming SSE).
- [ ] **T5.4** Build Opening & Book Explorer (`/openings`): White/Black ECO performance matrix, out-of-book depth, principle violation flags, and Elo-appropriate repertoire advice.
- [ ] **T5.5** Build Focus & Tilt Lab (`/focus`): Impulsivity scatter plot (move time vs. blunder rate), session tilt curve, and time-of-day accuracy heatmap.
- [ ] **T5.6** Build Personal Puzzle Trainer (`/puzzles`) and Endgame Sparring Arena (`/sparring`) with instant move feedback, hints, and FSRS rating controls.
- [ ] **T5.7** Build Road-to-GM & Goals Hub (`/roadmap`) + Notification Settings (`/settings`).
- [ ] **T5.8** Execute Impeccable audit cycle: `/impeccable critique`, `/impeccable polish`, `/impeccable audit`, `/impeccable harden`, and verify `npx impeccable detect src/` passes with 0 errors.

## Phase 6: Production Hardening
- [ ] **T6.1** Add Prometheus `/metrics` endpoint (Stockfish queue depth, avg analysis latency, OpenCode Go request latency & token usage).
- [ ] **T6.2** Add Postgres backup script (`scripts/backup.sh`) and deployment guide (`docs/DEPLOY.md`) for VPS / Coolify Docker Compose deployment.
- [ ] **T6.3** Run full end-to-end verification in Docker Compose (`docker compose up --build -d`) from fresh database through sync, analysis, reflection, puzzle solve, and endgame sparring.
