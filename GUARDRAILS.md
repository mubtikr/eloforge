# GUARDRAILS.md — Non-Negotiable Engineering, AI, Engine & UI Rules

## 1. Chess & AI Anti-Hallucination Guardrails (CRITICAL)
- **NEVER let an LLM evaluate a chess position, calculate variations, or decide if a move is a blunder.**
- Stockfish 17 + `shakmaty` are the **sole source of truth** for legality, evaluation (`cp`/mate), best moves, tactical tags, opening ECO codes, and phase classification.
- Every AI prompt sent to OpenCode Go MUST inject pre-computed, engine-verified JSON facts (FEN, legal SAN moves, exact eval swing, Stockfish PV, clock delta, mistake tags).
- Any move mentioned in AI coaching output must be validated against the provided PGN/PV context; instruct the model to never invent candidate variations outside the supplied Stockfish PV lines.

## 2. OpenCode Go Integration Guardrails
- **NEVER call OpenCode Go from the frontend browser** (CORS is blocked and API keys must never leak).
- **ALWAYS attach `x-opencode-session: <stable-uuid>`** and **`User-Agent: EloForge/1.0`** to every OpenCode Go request. Missing `x-opencode-session` causes HTTP 400 failures.
- **ALWAYS hit `https://opencode.ai/zen/go/v1`** (with `/v1`) and route to the model's exact protocol (`/chat/completions`, `/messages`, or `/responses`).
- Implement token-budget caps per user/day and circuit-breaker retries (backoff on `502`/`503`/`529`) to protect rolling OpenCode Go plan allowances.

## 3. Chess.com API & Legal Guardrails
- **NEVER send parallel burst requests to `api.chess.com/pub`.** Enforce a strict serial queue (concurrency = 1) with `ETag` / `If-None-Match` caching and respectful `429` backoff.
- **ALWAYS include contact info in the `User-Agent` header** (`EloForge/1.0 (contact@...)`).
- **Respect Chess.com IP:** Do NOT replicate Chess.com's board colors, piece sprites, sound effects, move classification icons, or trademarked feature names ("Game Review"). Display a clear non-affiliation disclaimer in the footer.

## 4. Stockfish Resource & Security Guardrails
- Never spawn unbounded Stockfish processes. All engine evaluations MUST acquire a permit from the bounded worker pool (`STOCKFISH_WORKERS <= num_cpus - 1`).
- Cap each Stockfish worker's hash table (`STOCKFISH_HASH_MB`, default `128`) and enforce strict per-position timeout (`<= 5000ms`) to prevent container OOM kills.
- Run Docker containers as **non-root users**. Never commit `.env`, API keys, or JWT secrets.
- Store session tokens in `HttpOnly; Secure; SameSite=Lax` cookies; validate all user-owned resources against `auth.user_id` to prevent IDOR.

## 5. UI Quality Guardrails (Impeccable)
- `npx impeccable detect src/` MUST pass with zero errors before merging any frontend milestone.
- Maintain WCAG AA contrast (`>= 4.5:1` body, `>= 3:1` large text), tabular numerals (`font-variant-numeric: tabular-nums`) for all ratings/clocks/evals, and full desktop + mobile responsiveness without stripping core review controls.
