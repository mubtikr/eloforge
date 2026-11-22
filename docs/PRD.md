# PRD.md — EloForge: Personal AI Chess Analyzer & Road-to-GM Coach

## 1. Product Vision & Target User
**Vision:** Transform raw Chess.com game history into an adaptive, closed-loop personal coaching system that diagnoses *why* a player loses (tactics, endgames, openings, or mental focus/tilt), generates training strictly from their own mistakes, remembers their thought process, and guides them rapidly from **500 → 1000+ Elo** and onward along a structured **Grandmaster Ladder**.

**North Star Metrics:**
- Reduction in 1-move hanging pieces per 10 games.
- Winning-position conversion rate (games won when reaching >= +4.0 eval).
- Impulsive blunder rate (blunders played in <5s with >50% clock left).
- Rating velocity (Elo gained per 30 days on Rapid).

---

## 2. Core Feature Modules

### Module A: One-Click Account Connection & Auto-Sync
- **Connect via Chess.com:** Username entry + 10-second verification challenge (with drop-in OAuth 2.0 upgrade path).
- **Smart Sync Engine:** Imports historical monthly archives serially with ETag caching, plus scheduled background polling (every 15 minutes or manual "Sync Now" button) to ingest newly finished games automatically.
- **Live Sync Stream:** Server-Sent Events (SSE) progress bar showing games fetched → Stockfish analyzed → puzzles extracted → AI report ready.

### Module B: Deterministic Stockfish Match Lab (No Hallucinations)
Every game is parsed and evaluated by Rust + Stockfish 17 before any AI sees it:
- **Move Classification (EPL-based):**
  - `Best` (EPL <= 0.01) | `Strong` (<= 0.03) | `Good` (<= 0.06) | `Inaccuracy` (<= 0.12) | `Mistake` (<= 0.22) | `Blunder` (> 0.22) | `Missed Win` (dropped from >85% win% to <55%).
- **Opening & Book Detection:** Matches exact ECO code (`A00`–`E99`), opening name, variation, and **Last Book Move**. Flags opening principle violations (uncastled past move 12, moving same piece 3+ times in first 10 moves, early queen sorties losing tempo).
- **Phase Segmentation:** Splits every game into **Opening**, **Middlegame**, and **Endgame** (triggered when non-pawn material drops below 13 points per side or queens are traded + <= 2 minor/major pieces) with separate Accuracy % per phase.
- **Deterministic Mistake Tagger:** Automatically labels every mistake/blunder with exact root tags:
  - `HANGING_PIECE`, `MISSED_FREE_PIECE`, `MISSED_MATE_1_2`, `ALLOWED_MATE`, `TACTICAL_BLINDNESS` (fork/pin/skewer/discovered attack), `WINNING_GAME_THROWN` (peaked >= +4.0 and didn't win), `STALEMATE_THROW`, `ENDGAME_TECHNIQUE_FAIL`, `OPENING_PRINCIPLE_BREAK`, `IMPULSIVE_BLUNDER`, `TIME_SCRAMBLE_BLUNDER`.

### Module C: Focus, Clock & Tilt Intelligence Engine
Uses `[%clk]` timestamps and game end timestamps from Chess.com PGNs to diagnose mental state:
- **Impulsivity Detector:** Flags moves played in `< 4 seconds` during Rapid games (10|0, 15|10) when >50% of time remains that result in a Mistake or Blunder.
- **Post-Blunder Cascade ("Mental Collapse"):** Detects when a player makes a second blunder within 3 moves of the first blunder.
- **Session Tilt Detector:** Groups games played within 30 minutes of each other. Flags losing streaks where accuracy drops >15% from Game 1 to Game 3+, recommending a strict "2-Loss Stop Rule".
- **Chronotype & Fatigue Map:** Tracks accuracy by hour-of-day (e.g., morning vs. late-night performance) and move-depth fatigue (moves 1–15 vs. 16–30 vs. 31+).

### Module D: Interactive Reflection & AI Coach Memory ("Tell Me Why")
- During Match Lab review, any critical moment has a **"Why did I play this?"** prompt (quick-select tags like *Didn't see defender*, *Tunnel vision on my attack*, *Panicked on clock*, *Forgot endgame pattern* + free-text explanation).
- The Rust backend bundles:
  1. Verified FEN, played move, Stockfish Top-3 lines, and exact mistake tags.
  2. Clock delta & focus state.
  3. The user's reflection + past similar reflections retrieved via PostgreSQL/`pgvector`.
- Sends this structured context to **OpenCode Go** (with a persistent `x-opencode-session` UUID).
- The AI Coach responds with:
  - Plain-English explanation tailored to the user's current rating band (no 2600-Elo jargon for a 500 player!).
  - A concrete **"Next-Time Trigger Rule"** (e.g., *"Before moving a knight forward, check if the square you left behind was defending your e4 pawn"*).
  - Saves the insight into the **Player Cognitive Profile** so future reports notice if the same mental habit repeats.

### Module E: Mistake-Derived Puzzles & Endgame Sparring Lab
1. **Personal Blunder Puzzles:** Every blunder (`EPL >= 0.15`) and missed tactic from your own games is automatically turned into a playable puzzle (showing the position *right before* your mistake).
2. **FSRS Spaced Repetition:** Schedules puzzles based on solve speed and hints used (1d → 3d → 7d → 21d) until the blind spot is permanently fixed.
3. **Endgame Sparring Arena ("Finish What You Started"):** Whenever the analyzer detects `WINNING_GAME_THROWN` or `ENDGAME_TECHNIQUE_FAIL`, it queues that exact FEN into a live practice board against a rating-calibrated Stockfish opponent so you replay and convert the winning endgame cleanly twice in a row.

### Module F: Road to 1000 → Grandmaster Ladder, Goals & Reminders
- **Adaptive Milestone Ladder:**
  - **Stage 1 (500 → 650): Foundation — Stop Giving Away Free Pieces**
    - *Unlock Criteria:* Hanging-piece rate < 1.5/game over last 10 Rapid games; 90% accuracy on Mate-in-1 puzzles; 3/3 wins in K+Q endgame drill.
  - **Stage 2 (650 → 800): Tactical Radar & King Safety**
    - *Unlock Criteria:* Castle before move 12 in 80% of games; Impulsive blunder rate < 15%; 20 personal blunder puzzles mastered.
  - **Stage 3 (800 → 1000): Conversion & Core Repertoire**
    - *Unlock Criteria:* Winning advantage (+4) conversion rate >= 75%; 1 White + 2 Black openings at >= 50% win rate; K+R & K+P endgame drills passed.
  - **Stage 4–8 (1000 → 1200 → 1500 → 1800 → 2200 → 2500 GM):** Progressive unlocks adding positional evaluation, pawn structures, rook endgames, deep calculation, and repertoire theory depth.
- **Dynamic "What to Learn Next" Prescriber:** After every 5–10 games, AI Coach ranks your **Top 3 Rating Leaks** (exact Elo cost per leak) and assigns this week's micro-curriculum (e.g., *Week Goal: Stop blowing winning endgames — complete 5 Endgame Sparring drills + study Lucena/King Opposition*).
- **Reminders & Nudges:** Configurable daily reminder schedule via **Web Push**, **Email (SMTP)**, and **Telegram Bot Webhook** (daily puzzle quota, milestone proximity alert, tilt warning after consecutive losses).

---

## 3. Complete Catalog of Reports (8 High-Impact Reports)
1. **Executive Weekly Coach Briefing (AI-Synthesized):** Rating trend, Top 3 Rating Leaks, biggest win of the week, recurring mental habits from your reflections, and exact 7-day action plan.
2. **Match Breakdown Report (Per-Game):** Eval graph, phase accuracy (Opening/Middlegame/Endgame), clock usage vs. eval chart, critical turning points, and interactive reflection thread.
3. **Opening Repertoire & Book Report:** Win/Draw/Loss & average accuracy by ECO opening (split by White vs. Black), "Out-of-Book" move number, most punished openings, and recommended repertoire simplifications for your Elo band.
4. **Weakness & Blunder Taxonomy Report:** Ranked frequency of every deterministic mistake tag (`HANGING_PIECE`, `MISSED_MATE`, `ENDGAME_TECHNIQUE_FAIL`, etc.) with trend arrows (improving vs. regressing).
5. **Conversion & Endgame Report:** Tracks every game where you reached `+3.0` or `+5.0` — what % you converted vs. threw away, categorized by endgame type (Queen, Rook, Minor Piece, Pawn-only) with 1-click "Replay vs Engine" launch.
6. **Focus, Speed & Tilt Report:** Impulsive blunders by move time, session tilt curves (Game 1 vs Game 2 vs Game 3+ win rate), time-of-day heatmap, and post-blunder cascade rate.
7. **Reflection & Habit Evolution Report:** Aggregates your "Why did I do this?" notes to show which psychological mistakes (tunnel vision, fear, haste) are shrinking vs. persisting.
8. **Road-to-GM Progress Report:** Visual ladder showing current Elo band, milestone checklists, projected days to next milestone (e.g., 1000 Rapid), and XP/streak consistency.
