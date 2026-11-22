import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Chess } from 'chess.js'
import { ArrowRight, Lightbulb } from 'lucide-react'
import { useMemo, useState, type FormEvent } from 'react'
import { Link } from 'react-router-dom'

import { api, getErrorMessage } from '../api/client'
import type { PuzzleAttemptResult, PuzzleDto, PuzzleStats } from '../api/types'
import Board, { type BoardArrow } from '../components/Board'
import { EmptyState, ErrorBlock, LoadingBlock, PageHeader, StatBlock, Tag } from '../components/ui'
import { formatPercent, tagLabel } from '../lib/format'

const RATINGS = [
  { value: 1, label: 'Again', note: 'failed' },
  { value: 2, label: 'Hard', note: 'with hints' },
  { value: 3, label: 'Good', note: 'solid' },
  { value: 4, label: 'Easy', note: 'instant' },
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
  const [hints, setHints] = useState(0)
  const [playedUci, setPlayedUci] = useState<string | null>(null)
  const [correct, setCorrect] = useState<boolean | null>(null)
  const [startedAt] = useState(() => Date.now())
  const [result, setResult] = useState<PuzzleAttemptResult | null>(null)
  const [showSolution, setShowSolution] = useState(false)
  const [uciInput, setUciInput] = useState('')

  const attempt = useMutation({
    mutationFn: (rating: number) =>
      api<PuzzleAttemptResult>(`/api/puzzles/${puzzle.id}/attempt`, {
        method: 'POST',
        body: JSON.stringify({
          uci: playedUci,
          hints,
          elapsed_ms: Date.now() - startedAt,
          rating,
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

  const dests = useMemo(() => destsFor(puzzle.fen), [puzzle])
  const orientation = puzzle.fen.split(' ')[1] === 'b' ? 'black' : 'white'

  const arrows: BoardArrow[] = useMemo(() => {
    const list: BoardArrow[] = []
    if (showSolution || result) {
      list.push({
        orig: puzzle.solution_uci.slice(0, 2),
        dest: puzzle.solution_uci.slice(2, 4),
        kind: 'best',
      })
    }
    if (playedUci && correct === false) {
      list.push({ orig: playedUci.slice(0, 2), dest: playedUci.slice(2, 4), kind: 'played' })
    }
    return list
  }, [puzzle, showSolution, result, playedUci, correct])

  function playMove(from: string, to: string, promotion = 'q') {
    if (result) return
    try {
      const chess = new Chess(puzzle.fen)
      const move = chess.move({ from, to, promotion })
      if (!move) return
      const uci = `${move.from}${move.to}${move.promotion ?? ''}`
      setPlayedUci(uci)
      const isCorrect = uci === puzzle.solution_uci
      setCorrect(isCorrect)
      if (!isCorrect) setShowSolution(true)
    } catch {
      // illegal move; chessground should prevent it, but keep the board intact
    }
  }

  function handleMove(from: string, to: string) {
    playMove(from, to)
  }

  function submitKeyboard(event: FormEvent) {
    event.preventDefault()
    const match = /^([a-h][1-8])([a-h][1-8])([qrbn])?$/i.exec(uciInput.trim())
    if (!match) return
    const from = match[1]
    const to = match[2]
    if (!from || !to) return
    playMove(from, to, match[3]?.toLowerCase() ?? 'q')
    setUciInput('')
  }

  return (
    <>
      <div className="puzzle-layout">
        <section className="puzzle-board">
          <Board
            fen={puzzle.fen}
            orientation={orientation}
            interactive={!result && correct === null}
            movableColor={orientation}
            dests={dests}
            onMove={handleMove}
            arrows={arrows}
            ariaLabel="Puzzle position. You play first."
          />
          <div className="board-controls">
            <span className={`result-word result-word--${orientation === 'white' ? 'win' : 'loss'}`}>
              {orientation === 'white' ? 'White' : 'Black'} to move
            </span>
            <span className="board-controls__spacer" />
            <button
              className="ef-btn ef-btn--ghost"
              type="button"
              onClick={() => {
                setHints((current) => current + 1)
                setShowSolution(true)
              }}
              disabled={Boolean(result)}
            >
              <Lightbulb size={14} />
              Hint {hints > 0 ? `(${hints})` : ''}
            </button>
          </div>
          <form className="kbd-move" onSubmit={submitKeyboard}>
            <label>
              <span className="ef-label">Keyboard move</span>
              <input
                className="ef-input kbd-move__input"
                value={uciInput}
                onChange={(event) => setUciInput(event.target.value)}
                placeholder="e2e4"
                maxLength={5}
                disabled={Boolean(result) || correct !== null}
                spellCheck={false}
              />
            </label>
            <button
              className="ef-btn ef-btn--ghost"
              type="submit"
              disabled={Boolean(result) || correct !== null || !uciInput.trim()}
            >
              Play move
            </button>
          </form>
        </section>

        <aside className="puzzle-panel">
          <p className="ef-label">Drill {index + 1} of {total} · {puzzle.kind.replaceAll('_', ' ')}</p>
          <p className="puzzle-context__players">
            {puzzle.white_username ?? 'White'} vs {puzzle.black_username ?? 'Black'}
          </p>
          <p className="ef-mono puzzle-context__epl">
            {puzzle.epl !== null
              ? `${formatPercent(puzzle.epl * 100, 1)} win probability lost`
              : 'mistake drill'}
          </p>
          <div className="match-tags">
            {puzzle.tags.map((tag) => (
              <Tag key={tag} tone="sienna">{tagLabel(tag)}</Tag>
            ))}
          </div>

          <div className="puzzle-state">
            {correct === null ? (
              <>
                <p className="ef-label">Your task</p>
                <p>Play the best move on the board. No clock, no shame — just the position.</p>
              </>
            ) : (
              <>
                <p className={`puzzle-verdict ${correct ? 'is-correct' : 'is-wrong'}`}>
                  {correct ? 'Correct. That was the engine move.' : 'Not the engine move.'}
                </p>
                <p>
                  Solution: <span className="ef-mono">{puzzle.solution_san ?? puzzle.solution_uci}</span>
                </p>
                {!result ? (
                  <>
                    <p className="ef-label">Rate the recall</p>
                    <div className="rating-grid">
                      {RATINGS.filter((rating) => correct || rating.value === 1).map((rating) => (
                        <button
                          key={rating.value}
                          type="button"
                          className="rating-btn"
                          onClick={() => attempt.mutate(rating.value)}
                          disabled={attempt.isPending}
                        >
                          <strong>{rating.label}</strong>
                          <span>{rating.note}</span>
                        </button>
                      ))}
                    </div>
                  </>
                ) : null}
              </>
            )}
            {attempt.error ? <p className="form-error">{getErrorMessage(attempt.error)}</p> : null}
            {result ? (
              <div className="puzzle-result">
                <p className="ef-label">Scheduled</p>
                <p className="ef-mono">{result.interval_days.toFixed(0)} day interval</p>
                <p className="ef-mono">stability {result.stability.toFixed(1)} · {result.state}</p>
                {result.mastered ? <Tag tone="good">mastered</Tag> : null}
                <button className="ef-btn ef-btn--ghost" type="button" onClick={nextPuzzle}>
                  Next drill <ArrowRight size={14} />
                </button>
              </div>
            ) : null}
          </div>

          <div className="puzzle-meta">
            <div><span className="ef-label">Drill type</span><span>{puzzle.kind === 'endgame_conversion' ? 'Endgame conversion' : 'Blunder fix'}</span></div>
            <div><span className="ef-label">Due</span><span className="ef-mono">{new Date(puzzle.due_at).toLocaleDateString('en-GB')}</span></div>
            <div><span className="ef-label">Reps</span><span className="ef-mono">{puzzle.reps}</span></div>
          </div>
        </aside>
      </div>
    </>
  )
}

export default function Puzzles() {
  const [index, setIndex] = useState(0)

  const due = useQuery({
    queryKey: ['puzzles', 'due'],
    queryFn: () => api<PuzzleDto[]>('/api/puzzles/due?limit=30'),
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

  return (
    <>
      <PageHeader
        eyebrow="Module E · Personal blunder puzzles"
        title="Find the move you missed."
        description="The position is from your own game, immediately before the error. Legal moves are enforced; the answer is Stockfish's top choice."
        actions={<Link className="ef-btn ef-btn--ghost" to="/sparring">Endgame Arena <ArrowRight size={14} /></Link>}
      />

      {!puzzle ? (
        <EmptyState title="The queue is clear" action={<Link className="ef-btn ef-btn--primary" to="/app">Analyse another game</Link>}>
          Every blunder with 15+ win-probability points lost becomes a drill. Analyse a game or let the daily queue refill tomorrow.
        </EmptyState>
      ) : (
        <PuzzleTrainer
          key={puzzle.id}
          puzzle={puzzle}
          index={safeIndex}
          total={puzzles.length}
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
