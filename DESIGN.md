# DESIGN.md — EloForge design system

## Direction
**A tactile editorial chess study room.** Warm paper, dark ink, walnut board, one burnt-sienna accent. It should feel like a well-kept scorebook and a serious newspaper chess column — not a SaaS dashboard and never Chess.com's green.

Three adjectives: **editorial, exact, warm.** Aesthetic risk: a large, high-contrast serif headline paired with hairline rules and mono numerals; the chessboard and eval curve are the only saturated objects on the page.

## Anti-patterns (never)
- Chess.com's greens, piece sprites, classification glyphs, or feature names.
- Purple/cyan AI gradients, neon glows, glassmorphism stacks.
- Pure `#000000` backgrounds; nested card-inside-card-inside-card layouts.
- Metric-card walls of equal weight; every number does not deserve a box.
- Rounded pill everything. Radii stay between 2px and 6px.
- Emoji as interface iconography.
- Fake precision or decorative charts without an axis or label.

## Palette
| Token | Value | Use |
| --- | --- | --- |
| `--ef-paper` | `#F3EDE0` | page ground |
| `--ef-paper-raised` | `#FAF6EC` | drawer / popover only |
| `--ef-paper-sunk` | `#E9E0CE` | inset wells |
| `--ef-ink` | `#242019` | primary text (warm off-black) |
| `--ef-ink-2` | `#5B5344` | secondary text |
| `--ef-ink-3` | `#6B6152` | captions, axis labels |
| `--ef-line` | `#D3C5AD` | hairlines, dividers |
| `--ef-sienna` | `#A64A24` | primary accent, user blunders |
| `--ef-sienna-deep` | `#7C361A` | accent hover/pressed |
| `--ef-brass` | `#B98A3E` | highlight, best move |
| `--ef-slate` | `#3F5560` | secondary series (clock) |
| `--ef-good` | `#4F6B4A` | "best/strong" only, never success chrome |
| `--ef-danger` | `#8E2F1C` | errors, blunder tags |
| board light | `#E7D7B6` | chessboard |
| board dark | `#8A6743` | chessboard |
| board highlight | `#C08A3E` | selected/last move ring |

Contrast: body text `--ef-ink` on `--ef-paper` ≥ 12:1. `--ef-ink-3` is only for ≥ 12px labels with 4.5:1 minimum; verify against `--ef-paper` before use. Accent text on paper uses `--ef-sienna-deep` for AA at small sizes.

## Typography
- Display: **Fraunces Variable** — headlines, page titles, large numbers. Optical size high, soft serifs, slight warmth. Weights 400/600.
- Body: **Instrument Sans Variable** — 15–17px, line-height 1.55. Weights 400/500/600.
- Data: **IBM Plex Mono** — Elo, clocks, centipawns, ECO codes, move notation. `font-variant-numeric: tabular-nums` everywhere numbers animate.
- Scale: 12 / 13 / 15 / 17 / 20 / 26 / 34 / 44 / 64 px. Display tracking −0.02em; mono tracking 0.
- Never set a paragraph in the display face. Never use all-caps for running text; small caps only for labels with 0.08em tracking.

## Spacing & layout
- 4px base. Section rhythm 24 / 40 / 72 / 112.
- Max reading width 68ch; app max width 1440px with 24px gutters (16px mobile).
- Desktop app shell: 232px left rail, content, optional right evidence rail (320px). Mobile: top bar + bottom tab bar; no hamburger maze.
- Hairline rules (`1px solid var(--ef-line)`) separate sections; cards are rare and flat, no shadow unless floating (drawer/popover). No card inside card.
- The board column is never smaller than 320px; on desktop it is the widest column.

## Components
- **Buttons:** 2px radius, 1px border, 40px min height, uppercase 11px tracking 0.08em labels for secondary; primary is solid ink, hover shifts to accent. Focus ring: 2px `--ef-sienna` offset 2px.
- **Evidence block:** label (small caps mono), value (Fraunces 34px), one-line interpretation. Used sparingly — max 3 per view.
- **Tags:** square-ish 2px radius, 11px mono uppercase; classification tags get a 3px left border, not a filled pill.
- **Tables:** no zebra stripes; 1px row rules; numeric columns right-aligned mono; sticky header on scroll for long lists.
- **Eval chart:** one continuous line (white advantage up, black down) filled at 8% opacity; clock as a dashed slate line on a secondary axis; critical plies marked with 4px squares, not dots.
- **Board:** walnut squares, cburnett pieces, last-move brass ring, best-move arrow in brass, played-blunder arrow in sienna. Coordinates in ink-3.
- **Drawer:** 380px from the right on desktop, full-height sheet on mobile; paper-raised, top hairline, no backdrop blur.

## Motion
- 150–220ms, `cubic-bezier(0.22, 1, 0.36, 1)`. Only transform/opacity. Board piece movement is chessground's own 200ms.
- Chart hover: no animated paths; the marker appears instantly. Numbers may count up once on mount, 400ms, and never re-run on scroll.
- Respect `prefers-reduced-motion`: transitions become 0ms, no count-up, no board animation.

## Copy rules
- Sentence case for headings, except one-line display headlines which may be all lowercase for texture.
- Use chess notation in mono. Always name the move, the evaluation swing, and the clock state.
- No anthropomorphising the engine ("Stockfish thinks you should..."), no shame ("you blundered again"), no hype.
- Empty states state the next action, not an apology.

## Accessibility
- WCAG AA: 4.5:1 body, 3:1 large text and UI borders. Verified with axe-core on Dashboard, Openings, Focus, Puzzles, Sparring, Roadmap, Settings and Match Lab (zero serious or critical findings).
- Every interactive element reachable by Tab, visible focus, board movable by keyboard (arrow keys to select, Enter to move) where feasible; otherwise a move list is the keyboard path.
- Charts have a text summary and a data table fallback (`aria-describedby`).
- Color is never the only signal: classification uses label + value + color.
