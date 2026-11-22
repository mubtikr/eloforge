# RESEARCH.md — Technical & Domain Feasibility

## 1. OpenCode Go Plan Integration
Verified OpenCode Go gateway requirements (as of late 2026):
- **Base URL:** `https://opencode.ai/zen/go/v1` (must include `/v1`; browser CORS is blocked, so all calls MUST proxy through the Rust backend).
- **Authentication:** `Authorization: Bearer <OPENCODE_API_KEY>` generated from `https://opencode.ai/auth`.
- **Mandatory Session Header:** Every request MUST include `x-opencode-session: <stable-uuid>`. Requests missing this header fail with `400 MissingSessionID`. Reuse one stable UUID per coaching conversation / game review to unlock prompt caching and routing affinity.
- **Mandatory Client Identification:** Set a custom `User-Agent: EloForge/1.0` (do NOT use default HTTP library strings like `reqwest` or `curl`).
- **Multi-Protocol Model Routing:** OpenCode Go exposes models across three upstream protocols (discoverable live at `GET https://opencode.ai/zen/go/v1/models`):
  1. `POST /chat/completions` (OpenAI Chat Completions): GLM, Kimi, DeepSeek, MiMo.
  2. `POST /messages` (Anthropic Messages): MiniMax, Qwen, Claude Haiku.
  3. `POST /responses` (OpenAI Responses): GPT Luna, Grok, Muse Spark.
- **Operational Quirks & Resilience:**
  - Always decode SSE streams as UTF-8 explicitly (charset header may be omitted).
  - Retry transient `502`, `503`, and `529` responses with exponential backoff + jitter.
  - Some models enforce or disallow `temperature`/`top_p`; keep sampling params configurable per model profile.
  - Because OpenCode Go enforces rolling usage windows, abstract inference behind an `LlmProvider` trait so OpenRouter, LiteLLM, or local Ollama can act as instant fallback.

## 2. Chess.com Authentication & PubAPI Reality
### Authentication ("Login with Chess.com")
- **Official OAuth 2.0:** Chess.com offers OAuth 2.0, but access is **application-gated** (requires approval via Chess.com's Developer Community OAuth application form). You cannot self-register a client ID instantly.
- **Recommended Strategy (Ship Day 1 + Upgrade Cleanly):**
  1. **Phase 1 (Immediate):** Implement **"Connect Chess.com"** via a cryptographic Bio/Location Challenge:
     - User clicks "Connect Chess.com" and enters username.
     - Backend generates a short nonce (e.g., `EF-7K9P`, expires in 10 min).
     - User pastes `EF-7K9P` into their Chess.com profile `Location` field (or `Name` field) and clicks Verify.
     - Backend calls `GET https://api.chess.com/pub/player/{username}`, verifies ownership, creates/signs in the user, and issues an `HttpOnly`, `Secure`, `SameSite=Lax` JWT session cookie. User can immediately delete the code.
     - Also allow instant "Read-Only Preview" for public analysis before verifying.
  2. **Phase 2 (Post-Approval):** Implement the `ChessComOAuthProvider` behind the same `AuthProvider` trait. Flip `CHESSCOM_OAUTH_ENABLED=true` once credentials arrive.

### Chess.com Public API (PubAPI) Rules
- **Base URL:** `https://api.chess.com/pub/` (read-only JSON-LD, no API key required).
- **Mandatory Header:** `User-Agent: EloForge/1.0 (contact: your-email@domain.com)`.
- **Key Endpoints:**
  - Profile: `GET /player/{username}`
  - Ratings/Stats: `GET /player/{username}/stats`
  - Monthly Archive Index: `GET /player/{username}/games/archives`
  - Monthly Games (includes full PGN + `[%clk]` timestamps): `GET /player/{username}/games/{YYYY}/{MM}`
- **Rate Limits & Caching:**
  - **Serial requests are unlimited; parallel requests trigger `429 Too Many Requests`.**
  - Enforce a strict serial worker/semaphore (concurrency = 1 per outbound PubAPI client) with `ETag` / `If-None-Match` headers so already-imported months return `304 Not Modified`.
- **IP & Branding Compliance:**
  - Never copy Chess.com's board green palette, piece designs, sound effects, move classification glyphs, or feature names (do NOT name your feature "Game Review"; use **"Match Lab"** or **"Coach Breakdown"**).
  - Include a clear footer: *"Independent training tool. Not affiliated with or endorsed by Chess.com."*

## 3. Chess Engine & Rust Ecosystem
- **Engine:** Official **Stockfish 17** binary (NNUE embedded) compiled/downloaded for the container CPU architecture (`x86_64-avx2` / `aarch64-armv8`) inside Docker.
- **Recommended Rust Crates:**
  - `shakmaty` + `shakmaty-syzygy`: Ultra-fast bitboard move generation, FEN/SAN/UCI validation, material counting, stalemate/checkmate checks, and optional 3-5 piece Syzygy endgame tablebases.
  - `pgn-reader`: Zero-allocation streaming PGN parser (extracts headers, SAN moves, and `[%clk H:MM:SS]` move clock comments).
  - `tokio::process`: Managed pool of long-lived Stockfish UCI child processes (avoid spawning a new process per move!).
- **Evaluation Calibration (Win Probability & Accuracy):**
  - Convert Stockfish centipawns (`cp`) to Win Probability (`0..100`) using the Lichess calibrated logistic curve:
    `win% = 50 + 50 * (2 / (1 + exp(-0.00368208 * cp)) - 1)`
  - Compute **Expected Points Lost (EPL)** per move (`win%_before - win%_after`).
  - Adaptive Depth Strategy (Max Speed + Accuracy):
    - Pass 1 (Full Game): `depth 14`, `MultiPV 2` (~1.5–3s per full 40-move game on 4 cores).
    - Pass 2 (Critical Positions where `EPL >= 0.10`): `depth 20`, `MultiPV 3` to guarantee accurate puzzle solutions and blunder explanations.

## 4. Impeccable Design Skill (`pbakaus/impeccable`)
- **What it is:** A design vocabulary, workflow skill, and anti-slop linter built specifically for AI coding agents (`https://impeccable.style`, `https://github.com/pbakaus/impeccable`).
- **Setup Commands:**
  - Install: `npx impeccable install` (or `npx skills add pbakaus/impeccable --skill impeccable`)
  - Initialize context: `/impeccable init` → creates `PRODUCT.md`
  - Lock design system: `/impeccable document` → creates `DESIGN.md`
- **Iterative Review Commands:** `/impeccable layout`, `/impeccable typeset`, `/impeccable colorize`, `/impeccable critique`, `/impeccable polish`, `/impeccable audit`, `/impeccable harden`, `/impeccable distill`.
- **Automated CI Gate:** `npx impeccable detect src/` — flags AI UI anti-patterns (purple/blue neon gradients, card-inside-card nesting, pure `#000` backgrounds, excessive glassmorphism, weak contrast, redundant metric cards).

## 5. Rating Improvement Science (500 → 1000+ → GM)
Data-backed bottlenecks by Elo band (so the coaching engine focuses on what actually moves rating):
- **500 → 700 (Blunder Elimination):** >70% of games are decided by 1-move hanging pieces, missed 1-move captures, and Mate-in-1. Deep opening theory is actively counterproductive here. Focus: **Blunder Check Habit** (Checks, Captures, Threats), zero impulsive moves, basic Queen/Rook ladder checkmates.
- **700 → 1000 (Tactical Vision & Conversion):** Games are decided by 2-move tactics (forks, pins, skewers, back-rank mates), leaving the king uncastled, and blowing +5 advantages into stalemates or dropped rooks. Focus: Opening *principles* (1 simple White setup + 2 Black setups), personal blunder puzzles via Spaced Repetition (FSRS), and Endgame Conversion drills.
- **1000 → 1500+ (Structured Chess):** Opening traps/repertoire consistency, pawn breaks, piece activity, Rook + Pawn endgames, time management.
- **1500 → 2500 (Mastery Ladder):** Deep calculation trees, prophylactic thinking, strategic imbalances, opponent prep, tournament stamina.
