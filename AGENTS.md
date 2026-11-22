# AGENTS.md — AI Coding Agent Operating Manual

## 1. Mission & Context
You are building **EloForge**, a high-performance Rust + Stockfish + React chess coaching platform.
Before writing code for any task, read:
1. `GUARDRAILS.md` (mandatory rules — never violate)
2. `docs/ARCHITECTURE.md` (system design & schema)
3. `docs/PRD.md` (exact feature & report specs)
4. `PLAN.md` and `TASKS.md` (pick the next unblocked task)

## 2. Engineering Workflow
1. **Plan First:** Before modifying >3 files, state the exact files to create/edit and verify against `docs/ARCHITECTURE.md`.
2. **Backend-First Contracts:** Define SQL migrations (`backend/migrations/`) and typed Rust DTOs first. Keep OpenAPI / TypeScript types synchronized (`ts-rs` or shared schema).
3. **Small Atomic Commits:** Complete one task ID from `TASKS.md` at a time and mark `[x]` only after all checks pass.
4. **Verification Before Completion:**
   - Rust: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
   - Frontend: `pnpm typecheck && pnpm lint && pnpm build && npx impeccable detect src/`
   - Docker: `docker compose config` (and full `docker compose up --build` at phase gates).

## 3. Impeccable Design Workflow (Mandatory for All Frontend Work)
This repository uses the **Impeccable** design skill (`pbakaus/impeccable`) to eliminate generic "AI slop" UI.
- **Phase 0 Setup:** Run `npx impeccable install`, then run `/impeccable init` (generating `PRODUCT.md`) and `/impeccable document` (generating `DESIGN.md`).
- **Aesthetic Direction for EloForge:**
  - **Editorial, Tactile Chess Study Room** — warm slate/off-black or parchment-cream high-contrast palette (NEVER Chess.com's green, NEVER purple/cyan AI neon gradients, NEVER pure `#000000`).
  - Distinctive display + readable sans + tabular numerals for Elo/clock/centipawns (avoid generic single-font Inter layouts).
  - The chessboard and eval timeline are the hero—avoid nested card-inside-card clutter.
- **After Every Major Frontend View:**
  1. Run `/impeccable critique <component/page>` and fix visual hierarchy/spacing issues.
  2. Run `/impeccable polish <component/page>` and `/impeccable audit` for WCAG AA contrast & keyboard navigation.
  3. Run `npx impeccable detect src/` — zero unresolved errors required to close a UI task.

## 4. Coding Conventions
- **Rust:** Zero `.unwrap()` or `.expect()` in production request/worker paths (use `thiserror` + `anyhow` / typed `AppError` -> HTTP JSON responses). Use `tracing` structured logs.
- **SQL:** Use parameterized `sqlx` queries only. Every list endpoint must be paginated.
- **TypeScript:** Strict mode (`noUncheckedIndexedAccess: true`), zero `any`, React Query for server state, Zustand only for ephemeral board UI state.
- **AI Prompts:** Store all prompt templates in versioned files under `backend/prompts/*.md` — never hardcode multi-line prompts inline in Rust handlers.