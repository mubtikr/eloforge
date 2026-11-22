# PRODUCT.md — EloForge

## What this is
EloForge is a personal chess study room: it imports a player's real Chess.com games, has Stockfish 17 evaluate every move, and turns the evidence into a closed coaching loop — diagnosis, explanation, drills made from your own blunders, and a visible road from 500 to 2500 Elo.

## Who it serves
- **Primary:** improving club players rated 500–1000 who lose most games to one-move blunders, unconverted winning positions, and tilt. They play Rapid on Chess.com; they do not read engine lines for fun.
- **Secondary:** 1000–1800 players who want a structured, data-backed study system without a human coach.

## The promise
Every sentence the coach says is grounded in Stockfish facts. The board, the evaluation curve and the clock are the primary evidence; the AI is an interpreter, never the judge.

## Core jobs
1. Connect a Chess.com account in under a minute (nonce challenge, no OAuth approval required).
2. Import every finished game serially, without hammering Chess.com.
3. Show a deterministic breakdown: classification, phase accuracy, opening/book, mistake tags, clock behaviour, tilt.
4. Let the player explain a move in their own words and receive one concrete next-time rule, remembered across games.
5. Turn every blunder into a spaced-repetition puzzle and every thrown endgame into a sparring session.
6. Show progress as a ladder of unlockable stages, not a vague "keep practicing".

## Non-goals
- Not a live game server, not an opening explorer for memorising 30 moves, not a replacement for a human coach.
- Never copy Chess.com's visual identity, vocabulary ("Game Review") or assets.
- Never let a language model calculate, evaluate, or invent chess facts.

## Voice
Plain, specific, unsentimental, a little dry. Address the move, not the person. No hype, no "unlock your potential", no exclamation marks. Use real numbers: "You gave away 2.4 pawns on move 18" beats "you made a mistake".

## Success signals
- Hanging-piece rate per 10 games falls month over month.
- Winning-position conversion (≥ +4.0) rises above 75%.
- Impulsive blunder rate falls below 15%.
- The player returns to the daily puzzle queue without being nudged.
- Weekly briefing is read, not skipped.

## Constraints
- Rust backend owns every truth: legality, evaluation, classifications, opening ECO, FSRS scheduling.
- Chess.com PubAPI is polled serially with ETags and a contactable User-Agent.
- OpenCode Go is server-side only, with a stable `x-opencode-session`, token budgets, and retries.
- WCAG AA, keyboard-first review controls, tabular numerals for all numbers that move.
- Self-hostable with Docker Compose on a modest VPS.
