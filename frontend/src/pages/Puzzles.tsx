import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Chess } from 'chess.js'
import { ArrowRight, Lightbulb, RotateCcw } from 'lucide-react'
import { useMemo, useState, type FormEvent } from 'react'
import { Link, useSearchParams } from 'react-router-dom'

import { api, getErrorMessage } from '../api/client'
import type {
  PuzzleAttemptResult,
  PuzzleDto,
  PuzzleExplanation,
  PuzzleStats,
} from '../api/types'
import Board, { type BoardArrow } from '../components/Board'
import { EmptyState, ErrorBlock, LoadingBlock, PageHeader, StatBlock, Tag } from '../components/ui'
import { formatDate, formatPercent, tagLabel } from '../lib/format'

const RATINGS = [
  { value: 1, label: 'Again', note: 'show me again soon' },
  { value: 2, label: 'Hard', note: 'solved with hints' },
  { value: 3, label: 'Good', note: 'solved with effort' },
  { value: 4, label: 'Easy', note: 'saw it instantly' },
] as const

const HINTS = [
  'Find the move that punishes the mistake.',
  'Look at the piece that can create the threat.',
  'The move starts on this square.',
  'The engine move is highlighted. Play it, then explain it.',
] as const

function destsFor(fen: string): Map<string, string[]> {
  try {
    const chess = new Chess(fen)
    const map = new Map<string, string[]>()
    for (const move of chess.moves({ verbose: true })) {
      const list = map.get(move.from) ?? []
      list.push(move.to)
      map.set(move.from, list)
    }
    return map
  } catch {
    return new Map()
  }
}

function fallbackGoal(tags: string[]): string {
  if (tags.some((tag) => ['MISSED_MATE_1_2', 'ALLOWED_MATE'].includes(tag))) {
    return 'Find the forced mate before the position slips.'
  }
  if (tags.some((tag) => ['HANGING_PIECE', 'MISSED_FREE_PIECE'].includes(tag))) {
    return 'Win the hanging material before it can be saved.'
  }
  if (tags.some((tag) => ['ENDGAME_TECHNIQUE_FAIL', 'WINNING_GAME_THROWN'].includes(tag))) {
    return 'Convert the winning endgame without allowing counterplay.'
  }
  return 'Find the move that changes the position in your favour.'
}

function solutionSquares(puzzle: PuzzleDto): { from: string; to: string; promotion?: string } {
  return {
    from: puzzle.solution_uci.slice(0, 2),
    to: puzzle.solution_uci.slice(2, 4),
    promotion: puzzle.solution_uci[4],
  }
}

function afterSolutionFen(puzzle: PuzzleDto): string {
  try {
    const chess = new Chess(puzzle.fen)
    const squares = solutionSquares(puzzle)
    const move = chess.move({ from: squares.from, to: squares.to, promotion: squares.promotion ?? 'q' })
    return move ? chess.fen() : puzzle.fen
  } catch {
    return puzzle.fen
  }
}

function PuzzleTrainer({
  puzzle,
  index,
  total,
  onNext,
}: {
  puzzle: PuzzleDto
  index: number
  total: number
  onNext: () => void
}) {
  const queryClient = useQueryClient()
  const [displayFen, setDisplayFen] = useState(puzzle.fen)
  const [lastMove, setLastMove] = useState<string | null>(null)
  const [playedUci, setPlayedUci] = useState<string | null>(null)
  const [correct, setCorrect] = useState<boolean | null>(null)
  const [wrongAttempts, setWrongAttempts] = useState(0)
  const [hintLevel, setHintLevel] = useState(0)
  const [revealed, setRevealed] = useState(false)
  const [startedAt] = useState(() => Date.now())
  const [result, setResult] = useState<PuzzleAttemptResult | null>(null)
  const [uciInput, setUciInput] = useState('')

  const explanation = useQuery({
    queryKey: ['puzzle', puzzle.id, 'explanation'],
    queryFn: () => api<PuzzleExplanation>(`/api/puzzles/${puzzle.id}/explanation`),
    retry: false,
  })

  const attempt = useMutation({
    mutationFn: (rating: number) =>
      api<PuzzleAttemptResult>(`/api/puzzles/${puzzle.id}/attempt`, {
        method: 'POST',
        body: JSON.stringify({
          uci: playedUci ?? puzzle.solution_uci,
          hints: hintLevel,
          elapsed_ms: Date.now() - startedAt,
          rating,
          revealed: revealed || correct !== true,
        }),
      }),
    onSuccess: (data) => {
      setResult(data)
      void queryClient.invalidateQueries({ queryKey: ['puzzles', 'stats'] })
      void queryClient.invalidateQueries({ queryKey: ['roadmap'] })
    },
  })

  const nextPuzzle = () => {
    void queryClient.invalidateQueries({ queryKey: ['puzzles', 'due'] })
    onNext()
  }

  const orientation = puzzle.fen.split(' ')[1] === 'b' ? 'black' : 'white'
  const solution = solutionSquares(puzzle)
  const showAnswer = correct === true || revealed || result !== null
  const dests = useMemo(() => destsFor(displayFen), [displayFen])
  const goal = explanation.data?.goal ?? fallbackGoal(puzzle.tags)

  const arrows: BoardArrow[] = useMemo(() => {
    const list: BoardArrow[] = []
    if (showAnswer || hintLevel >= 3) {
      list.push({ orig: solution.from, dest: solution.to, kind: 'best' })
    }
    if (playedUci && correct === false && !showAnswer) {
      list.push({ orig: playedUci.slice(0, 2), dest: playedUci.slice(2, 4), kind: 'played' })
    }
    return list
  }, [showAnswer, hintLevel, solution.from, solution.to, playedUci, correct])

  function applyMove(move: { from: string; to: string; promotion?: string }) {
    if (correct !== null || revealed || result) return
    try {
      const chess = new Chess(puzzle.fen)
      const played = chess.move(move)
      if (!played) return
      const uci = `${played.from}${played.to}${played.promotion ?? ''}`
      setDisplayFen(chess.fen())
      setLastMove(uci)
      setPlayedUci(uci)
      if (uci === puzzle.solution_uci) {
        setCorrect(true)
        setHintLevel((current) => Math.max(current, 1))
      } else {
        setCorrect(false)
        setWrongAttempts((current) => current + 1)
        setHintLevel((current) => Math.max(current, 1))
      }
    } catch {
      // illegal move; keep the position stable
    }
  }

  function resetPuzzle() {
    setDisplayFen(puzzle.fen)
    setLastMove(null)
    setPlayedUci(null)
    setCorrect(null)
  }

  function revealSolution() {
    setRevealed(true)
    setDisplayFen(afterSolutionFen(puzzle))
    setLastMove(puzzle.solution_uci)
    setPlayedUci(puzzle.solution_uci)
    setCorrect(false)
    setHintLevel(3)
  }

  function nextHint() {
    setHintLevel((current) => Math.min(3, current + 1))
  }

  function submitKeyboard(event: FormEvent) {
    event.preventDefault()
    const value = uciInput.trim()
    if (!value) return
    try {
      const chess = new Chess(displayFen)
      let played
      try {
        played = chess.move(value)
      } catch {
        const match = /^([a-h][1-8])([a-h][1-8])([qrbn])?$/i.exec(value)
        if (!match?.[1] || !match[2]) return
        played = chess.move({
          from: match[1],
          to: match[2],
          promotion: match[3]?.toLowerCase() ?? 'q',
        })
      }
      if (!played) return
      applyMove({ from: played.from, to: played.to, promotion: played.promotion })
      setUciInput('')
    } catch {
      // keep the input, let the user correct it
    }
  }

  const ratingOptions = correct === true
    ? hintLevel >= 2
      ? RATINGS.filter((rating) => rating.value <= 3)
      : RATINGS
    : [RATINGS[0]]

  return (
    <>
      <div className="puzzle-mobile-brief">
        <div className="puzzle-mobile-brief__head">
          <span className="ef-label">{orientation === 'white' ? 'White' : 'Black'} to move</span>
          {!showAnswer && correct === null ? (
            <button className="ef-btn ef-btn--ghost" type="button" onClick={nextHint}>
              <Lightbulb size={14} /> Hint {hintLevel > 0 ? `(${hintLevel}/3)` : ''}
            </button>
          ) : null}
        </div>
        <p>{goal}</p>
      </div>
      <div className="puzzle-layout">
        <section className="puzzle-board">
        <Board
          fen={displayFen}
          orientation={orientation}
          interactive={correct === null && !revealed && !result}
          movableColor={orientation}
          dests={dests}
          onMove={(from, to) => applyMove({ from, to, promotion: 'q' })}
          arrows={arrows}
          lastMove={lastMove ? [lastMove.slice(0, 2), lastMove.slice(2, 4)] : null}
          selectedSquare={hintLevel === 2 && correct === null ? solution.from : null}
          ariaLabel={
            showAnswer
              ? `Puzzle position after the solution ${puzzle.solution_san ?? puzzle.solution_uci}`
              : 'Puzzle position. Find the best move.'
          }
        />
        <div className="board-controls">
          <span className={`result-word result-word--${orientation === 'white' ? 'win' : 'loss'}`}>
            {orientation === 'white' ? 'White' : 'Black'} to move
          </span>
          <span className="board-controls__spacer" />
          {!showAnswer && correct === null ? (
            <button className="ef-btn ef-btn--ghost" type="button" onClick={nextHint}>
              <Lightbulb size={14} /> Hint {hintLevel > 0 ? `(${hintLevel}/3)` : ''}
            </button>
          ) : null}
          {correct === false && !revealed && !result ? (
            <button className="ef-btn ef-btn--ghost" type="button" onClick={resetPuzzle}>
              <RotateCcw size={14} /> Try again
            </button>
          ) : null}
        </div>
        {hintLevel > 0 && !showAnswer ? (
          <p className="puzzle-hint" role="status">
            <Lightbulb size={14} /> {HINTS[Math.min(hintLevel, HINTS.length - 1)]}
          </p>
        ) : null}
        <form className="kbd-move" onSubmit={submitKeyboard}>
          <label>
            <span className="ef-label">Keyboard move</span>
            <input
              className="ef-input kbd-move__input"
              value={uciInput}
              onChange={(event) => setUciInput(event.target.value)}
              placeholder="Nf3 or g4g5"
              maxLength={8}
              disabled={correct !== null || revealed || result !== null}
              spellCheck={false}
            />
          </label>
          <button
            className="ef-btn ef-btn--ghost"
            type="submit"
            disabled={correct !== null || revealed || result !== null || !uciInput.trim()}
          >
            Play move
          </button>
        </form>
      </section>

      <aside className="puzzle-panel">
        <div className="puzzle-panel__eyebrow">
          <span className="ef-label">Drill {index + 1} of {total}</span>
          <span className="ef-mono">{puzzle.kind === 'endgame_conversion' ? 'Endgame conversion' : 'Blunder fix'}</span>
        </div>
        <p className="puzzle-context__players">
          {puzzle.white_username ?? 'White'} vs {puzzle.black_username ?? 'Black'}
        </p>
        <p className="ef-mono puzzle-context__epl">
          {puzzle.epl !== null ? `${formatPercent(puzzle.epl * 100, 1)} win probability lost` : 'Mistake drill'}
        </p>
        <div className="match-tags">
          {puzzle.tags.map((tag) => <Tag key={tag} tone="sienna">{tagLabel(tag)}</Tag>)}
        </div>

        <div className="puzzle-task">
          <p className="ef-label">Goal</p>
          <p>{goal}</p>
        </div>

        <div className="puzzle-state" aria-live="polite">
          {correct === null && !revealed ? (
            <p className="puzzle-prompt">Play the best move. Hints are available if you need a nudge.</p>
          ) : null}
          {correct === false && !revealed ? (
            <>
              <p className="puzzle-verdict is-wrong">Not the engine move.</p>
              <p>
                You have tried {wrongAttempts} time{wrongAttempts === 1 ? '' : 's'}. Reset the position and look for a
                forcing move: checks, captures, threats.
              </p>
              <button className="ef-btn ef-btn--primary" type="button" onClick={resetPuzzle}>
                <RotateCcw size={14} /> Reset and try again
              </button>
              <button className="ef-btn ef-btn--ghost" type="button" onClick={revealSolution}>
                Show me the answer
              </button>
            </>
          ) : null}
          {correct === true || revealed ? (
            <div className="puzzle-answer">
              <p className={`puzzle-verdict ${correct === true ? 'is-correct' : 'is-wrong'}`}>
                {correct === true ? 'Correct. Now understand why.' : 'Solution revealed.'}
              </p>
              <p>
                Engine move: <span className="ef-mono">{puzzle.solution_san ?? puzzle.solution_uci}</span>
              </p>
              {explanation.isLoading ? <p className="ef-mono">Loading the explanation…</p> : null}
              {explanation.data ? (
                <>
                  <p className="puzzle-why">{explanation.data.why}</p>
                  {explanation.data.opponent_reply_san ? (
                    <p>
                      Expected reply: <span className="ef-mono">{explanation.data.opponent_reply_san}</span>
                    </p>
                  ) : null}
                  {explanation.data.best_line_san.length > 1 ? (
                    <p className="puzzle-line ef-mono">{explanation.data.best_line_san.join(' · ')}</p>
                  ) : null}
                  <div className="next-rule lesson-rule">
                    <p className="ef-label">Next-time rule</p>
                    <p>{explanation.data.next_time_rule}</p>
                  </div>
                </>
              ) : explanation.isError ? (
                <p className="form-error">{getErrorMessage(explanation.error)}</p>
              ) : null}
            </div>
          ) : null}

          {!result && (correct === true || revealed) ? (
            <div className="puzzle-rate">
              <p className="ef-label">How did it feel?</p>
              <p className="puzzle-rate__help">
                The rating schedules the next review. Use Again when you needed the answer, Hard when you used hints,
                Good when you worked it out, Easy when it was immediate.
              </p>
              <div className="rating-grid">
                {ratingOptions.map((rating) => (
                  <button
                    key={rating.value}
                    type="button"
                    className="rating-btn"
                    onClick={() => attempt.mutate(rating.value)}
                    disabled={attempt.isPending || explanation.isLoading}
                  >
                    <strong>{rating.label}</strong>
                    <span>{rating.note}</span>
                  </button>
                ))}
              </div>
            </div>
          ) : null}
          {attempt.error ? <p className="form-error">{getErrorMessage(attempt.error)}</p> : null}
          {result ? (
            <div className="puzzle-result">
              <p className="ef-label">Next review</p>
              <p className="ef-mono">{result.interval_days.toFixed(0)} day interval · {formatDate(result.due_at)}</p>
              <p className="ef-mono">stability {result.stability.toFixed(1)} · {result.state}</p>
              {result.mastered ? <Tag tone="good">mastered</Tag> : null}
              <button className="ef-btn ef-btn--ghost" type="button" onClick={nextPuzzle}>
                Next drill <ArrowRight size={14} />
              </button>
            </div>
          ) : null}
        </div>

        <div className="puzzle-meta">
          <div><span className="ef-label">Due</span><span className="ef-mono">{formatDate(puzzle.due_at)}</span></div>
          <div><span className="ef-label">Reps</span><span className="ef-mono">{puzzle.reps}</span></div>
          {puzzle.game_id ? (
            <Link className="puzzle-source" to={`/games/${puzzle.game_id}`}>
              Review the source game <ArrowRight size={13} />
            </Link>
          ) : null}
        </div>
        </aside>
      </div>
    </>
  )
}

export default function Puzzles() {
  const [searchParams] = useSearchParams()
  const tag = searchParams.get('tag')
  const [index, setIndex] = useState(0)

  const due = useQuery({
    queryKey: ['puzzles', 'due', tag],
    queryFn: () =>
      api<PuzzleDto[]>(`/api/puzzles/due?limit=5${tag ? `&tag=${encodeURIComponent(tag)}` : ''}`),
  })
  const stats = useQuery({
    queryKey: ['puzzles', 'stats'],
    queryFn: () => api<PuzzleStats>('/api/puzzles/stats'),
  })

  if (due.isLoading || stats.isLoading) return <LoadingBlock label="Building today's drill set…" />
  if (due.isError) return <ErrorBlock message={getErrorMessage(due.error)} retry={() => void due.refetch()} />

  const puzzles = due.data ?? []
  const safeIndex = puzzles.length === 0 ? 0 : Math.min(index, puzzles.length - 1)
  const puzzle = puzzles[safeIndex] ?? null
  const title = tag ? `${tagLabel(tag)} drills` : 'Find the move you missed.'

  return (
    <>
      <PageHeader
        className="game-page puzzle-page"
        eyebrow={tag ? `Focused practice · ${tagLabel(tag)}` : 'Module E · Personal blunder puzzles'}
        title={title}
        description={
          tag
            ? `A focused set built from your ${tagLabel(tag).toLowerCase()} mistakes. Clear the filter to return to the full queue.`
            : 'The position is from your own game, immediately before the error. Legal moves are enforced; the answer is Stockfish’s top choice.'
        }
        actions={
          tag ? (
            <Link className="ef-btn ef-btn--ghost" to="/puzzles">
              Clear filter <RotateCcw size={14} />
            </Link>
          ) : (
            <Link className="ef-btn ef-btn--ghost" to="/sparring">
              Endgame Arena <ArrowRight size={14} />
            </Link>
          )
        }
      />

      <div className="puzzle-progress">
        <span className="ef-label">Today’s set</span>
        <div
          className="progress-rule"
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={Math.min(puzzles.length, 5)}
          aria-valuenow={puzzle ? safeIndex + 1 : 0}
          aria-label="Daily drill set progress"
        >
          <span style={{ width: `${puzzles.length ? ((safeIndex + 1) / Math.min(puzzles.length, 5)) * 100 : 0}%` }} />
        </div>
        <span className="ef-mono">
          {puzzle ? `${safeIndex + 1} of ${Math.min(puzzles.length, 5)}` : 'clear'}
        </span>
      </div>

      {!puzzle ? (
        <EmptyState title={tag ? 'No due drills in this leak' : 'The queue is clear'} action={<Link className="ef-btn ef-btn--primary" to="/app">Analyse another game</Link>}>
          {tag
            ? 'This leak has no due drills right now. Clear the filter or review a game to create a fresh one.'
            : 'Every blunder with 15+ win-probability points lost becomes a drill. Analyse a game or let the daily queue refill tomorrow.'}
        </EmptyState>
      ) : (
        <PuzzleTrainer
          key={puzzle.id}
          puzzle={puzzle}
          index={safeIndex}
          total={Math.min(puzzles.length, 5)}
          onNext={() => setIndex((current) => (current + 1 < puzzles.length ? current + 1 : 0))}
        />
      )}

      <section className="puzzle-stats">
        <StatBlock label="Due now" value={stats.data?.due ?? 0} tone="brass" />
        <StatBlock label="Total drills" value={stats.data?.total ?? 0} />
        <StatBlock label="Mastered" value={stats.data?.mastered ?? 0} tone="good" />
        <StatBlock label="Endgame drills" value={stats.data?.endgame_conversion ?? 0} />
      </section>
    </>
  )
}
