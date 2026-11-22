# PLAN.md — Phased Implementation Blueprint

## Phase 0: Foundation, Tooling & Design System (Days 1–2)
**Goal:** Bootable monorepo, Postgres/Valkey/Rust/Vite Docker stack, and Impeccable design foundation.
- Initialize Cargo workspace (`backend/`) and Vite React TS app (`frontend/`).
- Install Impeccable (`npx impeccable install`), generate `PRODUCT.md` and `DESIGN.md`.
- Stand up `docker-compose.yml` with `postgres` (pgvector), `valkey`, `api` (with Stockfish 17 binary verified via `stockfish uci`), and `web` (Caddy).
- **Exit Gate:** `docker compose up --build` starts cleanly; `/api/healthz` confirms DB + Valkey + Stockfish worker pool online.

## Phase 1: Chess.com Auth & Serial Game Sync Engine (Days 3–5)
**Goal:** One-click Chess.com connection and full game archive ingestion.
- Implement `AuthProvider` trait + Chess.com Profile Verification Challenge + JWT HttpOnly session cookies (+ stubbed `ChessComOAuthProvider` ready for credentials).
- Build serial rate-limited Chess.com PubAPI client with `ETag` caching and `sync_cursors`.
- Stream PGNs with `pgn-reader` (extracting moves, headers, and `[%clk]` timestamps) into `games` and `game_moves`.
- **Exit Gate:** User connects their real Chess.com account, imports 100+ games with zero `429` errors, and sees live sync progress via SSE.

## Phase 2: Stockfish Worker Pool & Deterministic Analyzer (Days 6–10)
**Goal:** Fast, accurate game analysis, ECO opening detection, mistake tagging, and Focus/Tilt telemetry.
- Build managed Stockfish 17 UCI worker pool + Valkey analysis queue.
- Embed Lichess ECO opening database (`A00`–`E99`) for exact opening & last-book-move detection.
- Implement Win%, EPL, Move Classification, Phase Segmentation (Opening/Middlegame/Endgame), and Deterministic Mistake Tagger (`HANGING_PIECE`, `WINNING_GAME_THROWN`, `ENDGAME_TECHNIQUE_FAIL`, `IMPULSIVE_BLUNDER`, etc.).
- Build Focus & Tilt analyzer (session clustering, impulsive move ratio, post-blunder cascade, chronotype heatmap).
- **Exit Gate:** A 40-move game analyzes in <5s; all 8 deterministic report queries return accurate metrics on real imported games.

## Phase 3: OpenCode Go AI Coach & Reflection Memory Loop (Days 11–14)
**Goal:** Reasoning coach that explains mistakes in 500-to-1000 language and remembers your "Why I played this" notes.
- Build `OpenCodeGoProvider` (`https://opencode.ai/zen/go/v1`, `x-opencode-session`, custom `User-Agent`, multi-protocol router).
- Create versioned prompt templates (`game_review.md`, `reflection_response.md`, `weekly_coach_report.md`, `curriculum_prescriber.md`).
- Implement `move_reflections` API + semantic/tag memory lookup so the coach connects recurring mental mistakes across games.
- **Exit Gate:** Clicking any blunder in Match Lab allows submitting a reflection and streams back an engine-grounded, personalized rule via OpenCode Go.

## Phase 4: Mistake-Derived Puzzles, Endgame Sparring & Road-to-GM Ladder (Days 15–19)
**Goal:** Active training loop that turns weaknesses into mastery.
- Auto-extract puzzles from every blunder (`EPL >= 0.15`) and blown winning endgame.
- Implement FSRS Spaced Repetition scheduler for daily puzzle queues.
- Build live **Endgame Sparring Arena** (play out your own failed winning endgames against rating-calibrated Stockfish until converted).
- Build **Road-to-GM Ladder** (500 → 650 → 800 → 1000 → ... → 2500), auto-evaluated milestones, and Goal/Reminder scheduler (Telegram webhook + SMTP + Web Push).
- **Exit Gate:** Newly analyzed blunder immediately appears in Daily Puzzles; failed winning endgame launches in Sparring Arena; milestone progress updates automatically.

## Phase 5: Frontend Match Lab, 8 Reports & Impeccable Polish (Days 20–24)
**Goal:** Fast, joyful, zero-slop UI across Desktop & Mobile.
- Build Dashboard, Match Lab (60fps `chessground` + eval/clock dual chart + reflection drawer), Opening Book Explorer, Focus/Tilt Lab, Puzzle Trainer, Endgame Sparring, and Road-to-GM view.
- Run full Impeccable design pass (`/impeccable critique`, `polish`, `audit`, `harden`) and enforce `npx impeccable detect src/` clean output.
- **Exit Gate:** Lighthouse Performance > 90, 60fps move scrubbing, zero Impeccable detector errors.

## Phase 6: Production Hardening & Observability (Days 25–26)
**Goal:** Reliable self-hosted production deployment.
- Add structured JSON logging, Prometheus `/metrics` (queue depth, Stockfish latency, OpenCode token usage), automated Postgres backup script, and Coolify/Docker Compose deployment docs.
- **Exit Gate:** Fresh `docker compose up -d` on a clean VPS passes full end-to-end smoke test.
