# ARCHITECTURE.md — System Design & Production Topology

## 1. High-Level Architecture
```text
[Browser: Vite + React + chessground]
       │ HTTPS / SSE (via Caddy or Nginx reverse proxy)
       ▼
[Rust API Server (Axum + Tokio)]
   ├── Auth Service (Bio-Challenge + Pluggable Chess.com OAuth2)
   ├── Chess.com Sync Worker (Serial Rate-Limited HTTP + ETag Cache)
   ├── Stockfish Pool Manager (Bounded N UCI Stockfish 17 Processes)
   ├── Deterministic Analyzer (shakmaty + ECO Matcher + Mistake/Tilt Tagger)
   ├── Puzzle & FSRS Scheduler + Endgame Sparring Engine
   ├── AI Coach Client (OpenCode Go Gateway + Session UUID Manager)
   └── Reminder Scheduler (Cron -> Telegram / Email / Web Push)
       │                                      │
       ▼                                      ▼
[PostgreSQL 17 + pgvector]            [Valkey 8 (Queues + PubSub + Rate Locks)]
