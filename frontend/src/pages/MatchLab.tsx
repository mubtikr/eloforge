import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Chess } from 'chess.js'
import {
  ChevronLeft,
  ChevronRight,
  Download,
  Eye,
  MessageSquareQuote,
  Sparkles,
  Target,
} from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { Link, useParams } from 'react-router-dom'

import { api, getErrorMessage } from '../api/client'
import type { AnalyzeResponse, GameDetail, MoveDto } from '../api/types'
import Board, { type BoardArrow } from '../components/Board'
import EvalChart from '../components/EvalChart'
import ReflectDrawer from '../components/ReflectDrawer'
import { EmptyState, ErrorBlock, LoadingBlock, Tag } from '../components/ui'
import { explainMove } from '../lib/explain'
import {
  classificationClass,
  classificationLabel,
  formatClock,
  formatDate,
  formatEval,
  formatPercent,
  tagLabel,
  uciToSquares,
} from '../lib/format'

interface AccuracyShape {
  user?: number
  white?: number
  black?: number
  opening?: number
  middlegame?: number
  endgame?: number
}

interface Violation {
  code: string
  label: string
  detail: string
  ply: number
}

interface AnalysisShape {
  accuracy?: AccuracyShape
  violations?: Violation[]
  game_tags?: string[]
  turning_points?: Array<{ ply: number; san: string; classification: string; epl: number }>
}

type ReviewMode = 'review' | 'moves' | 'report'

function isBad(move: MoveDto): boolean {
  return ['inaccuracy', 'mistake', 'blunder', 'missed_win'].includes(move.classification ?? '')
}

function isCritical(move: MoveDto): boolean {
  return ['mistake', 'blunder', 'missed_win'].includes(move.classification ?? '')
}

function boardCheck(fen: string): boolean {
  try {
    return new Chess(fen).isCheck()
  } catch {
    return false
  }
}

export default function MatchLab() {
  const { id } = useParams<{ id: string }>()
  const queryClient = useQueryClient()
  const [selection, setSelection] = useState<{ gameId: string | undefined; ply: number }>({
    gameId: undefined,
    ply: 0,
  })
  const [mode, setMode] = useState<ReviewMode>('review')
  const [showBest, setShowBest] = useState(false)
  const [reflectOpen, setReflectOpen] = useState(false)
  const listRef = useRef<HTMLDivElement>(null)

  const gameQuery = useQuery({
    queryKey: ['game', id],
    queryFn: () => api<GameDetail>(`/api/games/${id}`),
    enabled: Boolean(id),
  })

  const analyze = useMutation({
    mutationFn: () => api<AnalyzeResponse>(`/api/games/${id}/analyze`, { method: 'POST' }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['game', id] })
      void queryClient.invalidateQueries({ queryKey: ['games'] })
    },
  })

  const game = gameQuery.data
  const moves = game?.moves ?? []
  const analysis = (game?.analysis ?? null) as AnalysisShape | null
  const accuracy = analysis?.accuracy
  const userColor = game?.game.user_color === 'black' ? 'black' : 'white'

  const criticalMoments = moves.filter(
    (move) => move.color === game?.game.user_color && isCritical(move),
  )
  const firstCritical = criticalMoments[0]?.ply
  const defaultPly = firstCritical ?? moves[moves.length - 1]?.ply ?? 0

  const selectedPly = selection.gameId === id ? selection.ply : defaultPly
  function selectPly(next: number | ((current: number) => number)) {
    setShowBest(false)
    setSelection((previous) => {
      const base = previous.gameId === id ? previous.ply : defaultPly
      const value = typeof next === 'function' ? next(base) : next
      return { gameId: id, ply: Math.max(0, Math.min(moves.length, value)) }
    })
  }

  const selectedMove: MoveDto | null = moves.find((move) => move.ply === selectedPly) ?? null
  const previousMove: MoveDto | null = moves.find((move) => move.ply === selectedPly - 1) ?? null
  const lesson = selectedMove && game ? explainMove(selectedMove, game.game) : null
  const displayFen = selectedMove
    ? showBest
      ? selectedMove.fen_before
      : selectedMove.fen_after
    : game?.initial_fen ?? ''
  const boardLastMove = showBest
    ? uciToSquares(previousMove?.uci)
    : uciToSquares(selectedMove?.uci)
  const check = boardCheck(displayFen)
  const currentCriticalIndex = criticalMoments.findIndex((move) => move.ply === selectedPly)

  useEffect(() => {
    if (!game) return
    const handler = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null
      if (target && ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName)) return
      if (reflectOpen) return
      if (event.key === 'ArrowLeft') {
        event.preventDefault()
        setShowBest(false)
        setSelection({ gameId: id, ply: Math.max(0, selectedPly - 1) })
      } else if (event.key === 'ArrowRight') {
        event.preventDefault()
        setShowBest(false)
        setSelection({ gameId: id, ply: Math.min(moves.length, selectedPly + 1) })
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [game, id, moves.length, reflectOpen, selectedPly])

  useEffect(() => {
    if (mode !== 'moves') return
    const node = listRef.current?.querySelector<HTMLElement>(`[data-ply="${selectedPly}"]`)
    node?.scrollIntoView({ block: 'nearest' })
  }, [mode, selectedPly])

  const arrows: BoardArrow[] = useMemo(() => {
    if (!selectedMove) return []
    const list: BoardArrow[] = []
    if (selectedMove.uci) {
      const played = uciToSquares(selectedMove.uci)
      if (played) list.push({ orig: played[0], dest: played[1], kind: 'played' })
    }
    if (showBest && selectedMove.best_move_uci) {
      const best = uciToSquares(selectedMove.best_move_uci)
      if (best) list.push({ orig: best[0], dest: best[1], kind: 'best' })
    }
    return list
  }, [selectedMove, showBest])

  if (gameQuery.isLoading) return <LoadingBlock label="Loading the review room…" />
  if (gameQuery.isError) {
    return <ErrorBlock message={getErrorMessage(gameQuery.error)} retry={() => void gameQuery.refetch()} />
  }
  if (!game) return null

  const opponent =
    game.game.user_color === 'white' ? game.game.black_username : game.game.white_username
  const openingViolations = analysis?.violations ?? []
  const moveTags = selectedMove?.mistake_tags ?? []
  const phases = [
    { key: 'opening', label: 'Opening', value: accuracy?.opening },
    { key: 'middlegame', label: 'Middlegame', value: accuracy?.middlegame },
    { key: 'endgame', label: 'Endgame', value: accuracy?.endgame },
  ]
  const practiceTag = moveTags[0]
  const hasNextCritical = criticalMoments.length > 0

  function goToNextCritical() {
    if (criticalMoments.length === 0) return
    const nextIndex =
      currentCriticalIndex >= 0 && currentCriticalIndex + 1 < criticalMoments.length
        ? currentCriticalIndex + 1
        : 0
    const target = criticalMoments[nextIndex]
    if (target) {
      setMode('review')
      selectPly(target.ply)
    }
  }

  return (
    <>
      <header className="match-head">
        <div>
          <p className="ef-label">
            {game.game.time_class ?? 'chess'} · {formatDate(game.game.start_time)} · {game.game.opening_name ?? game.game.opening_eco ?? 'Unknown opening'}
          </p>
          <h1 className="match-head__title">
            {game.game.white_username}
            <span className="match-head__vs"> vs </span>
            {game.game.black_username}
          </h1>
          <div className="match-head__meta">
            <span className={`result-word result-word--${game.game.result}`}>
              {game.game.result.toUpperCase()}
            </span>
            <span className="ef-mono">
              {game.game.white_rating ?? '—'} / {game.game.black_rating ?? '—'}
            </span>
            <span>{game.game.time_control ?? '—'}</span>
            <Tag tone={game.game.analysis_status === 'done' ? 'good' : 'neutral'}>
              {game.game.analysis_status}
            </Tag>
          </div>
        </div>
        <div className="match-head__actions">
          <a className="ef-btn ef-btn--ghost" href={`/api/games/${game.game.id}/pgn`}>
            <Download size={15} /> PGN
          </a>
          {game.game.analysis_status !== 'done' ? (
            <button
              className="ef-btn ef-btn--primary"
              type="button"
              onClick={() => analyze.mutate()}
              disabled={analyze.isPending}
            >
              <Sparkles size={15} />
              {analyze.isPending ? 'Queuing…' : 'Analyze now'}
            </button>
          ) : null}
        </div>
      </header>

      {game.game.analysis_status !== 'done' ? (
        <div className="match-notice">
          <span className="ef-label">Analysis {game.game.analysis_status}</span>
          <p>
            The board is available; classifications and coaching appear when Stockfish finishes.
            {game.game.analysis_status === 'failed' ? ' Retry above.' : ' The review will fill in automatically.'}
          </p>
        </div>
      ) : null}

      <div className="review-toolbar">
        <div className="review-tabs" role="tablist" aria-label="Review mode">
          {(
            [
              ['review', 'Guided review'],
              ['moves', 'All moves'],
              ['report', 'Report'],
            ] as const
          ).map(([key, label]) => (
            <button
              key={key}
              type="button"
              role="tab"
              aria-selected={mode === key}
              className={`review-tab${mode === key ? ' is-active' : ''}`}
              onClick={() => setMode(key)}
            >
              {label}
            </button>
          ))}
        </div>
        <div className="review-toolbar__actions">
          {selectedMove && selectedMove.best_move_uci ? (
            <button
              className={`ef-btn ef-btn--ghost${showBest ? ' is-on' : ''}`}
              type="button"
              aria-pressed={showBest}
              onClick={() => setShowBest((current) => !current)}
            >
              <Eye size={14} />
              {showBest ? 'Showing before + best' : 'Show best move'}
            </button>
          ) : null}
          {criticalMoments.length > 0 ? (
            <button className="ef-btn ef-btn--ghost" type="button" onClick={goToNextCritical}>
              <Target size={14} />
              {currentCriticalIndex >= 0 ? 'Next critical moment' : 'First critical moment'}
            </button>
          ) : null}
        </div>
      </div>

      <div className="match-layout">
        <section className="match-board">
          <Board
            fen={displayFen}
            orientation={userColor}
            lastMove={boardLastMove}
            arrows={arrows}
            check={check}
            ariaLabel={
              selectedMove
                ? `${showBest ? 'Position before' : 'Position after'} ${selectedMove.move_number}. ${selectedMove.san}`
                : 'Starting position'
            }
          />
          <div className="board-controls">
            <button
              className="icon-btn board-step"
              type="button"
              onClick={() => selectPly((current) => Math.max(0, current - 1))}
              aria-label="Previous move"
            >
              <ChevronLeft size={18} />
            </button>
            <span className="ef-mono board-controls__label">
              {selectedMove
                ? `${selectedMove.move_number}. ${selectedMove.san} · ${showBest ? 'before' : 'after'}`
                : 'Start position'}
            </span>
            <button
              className="icon-btn board-step"
              type="button"
              onClick={() => selectPly((current) => Math.min(moves.length, current + 1))}
              aria-label="Next move"
            >
              <ChevronRight size={18} />
            </button>
            <span className="board-controls__spacer" />
            <span className="ef-mono board-controls__clock">
              {formatClock(showBest ? previousMove?.clock_remaining_sec : selectedMove?.clock_remaining_sec)}
            </span>
          </div>
          <div className="match-evidence">
            <span><i className="swatch swatch--played" /> played move</span>
            {showBest ? <span><i className="swatch swatch--best" /> engine best</span> : null}
            <span className="ef-mono">
              {showBest ? 'before position' : 'after position'}
            </span>
            <span className="ef-mono">← → to step</span>
          </div>
        </section>

        <section className="match-panel" role="tabpanel">
          {mode === 'review' ? (
            <div className="lesson-panel">
              {selectedMove && lesson ? (
                <>
                  <div className="lesson-panel__head">
                    <div>
                      <p className="ef-label">Move {selectedMove.move_number}</p>
                      <p className="lesson-panel__san ef-mono">{selectedMove.san}</p>
                    </div>
                    <div className="lesson-panel__badges">
                      <span className={`class-pill ${classificationClass(selectedMove.classification)}`}>
                        {classificationLabel(selectedMove.classification)}
                      </span>
                      <span className="ef-mono lesson-panel__swing">
                        {(selectedMove.epl ?? 0) * 100 > 0 ? '−' : ''}
                        {formatPercent((selectedMove.epl ?? 0) * 100, 1)} win
                      </span>
                    </div>
                  </div>
                  <div className="lesson-block">
                    <p className="ef-label">What happened</p>
                    <p>{lesson.what}</p>
                  </div>
                  <div className="lesson-block">
                    <p className="ef-label">Why it happened</p>
                    <p>{lesson.why}</p>
                  </div>
                  <div className="next-rule lesson-rule">
                    <p className="ef-label">Next-time rule</p>
                    <p>{lesson.rule}</p>
                  </div>
                  <div className="lesson-actions">
                    {selectedMove.color === game.game.user_color && isBad(selectedMove) ? (
                      <button className="ef-btn ef-btn--primary" type="button" onClick={() => setReflectOpen(true)}>
                        <MessageSquareQuote size={15} />
                        Reflect on this move
                      </button>
                    ) : null}
                    {practiceTag ? (
                      <Link className="ef-btn ef-btn--ghost" to={`/puzzles?tag=${encodeURIComponent(practiceTag)}`}>
                        Practise this leak
                      </Link>
                    ) : (
                      <Link className="ef-btn ef-btn--ghost" to="/puzzles">
                        Open the puzzle trainer
                      </Link>
                    )}
                  </div>
                  {moveTags.length > 0 ? (
                    <div className="match-tags">
                      {moveTags.map((tag) => <Tag key={tag} tone="sienna">{tagLabel(tag)}</Tag>)}
                    </div>
                  ) : null}
                </>
              ) : (
                <EmptyState title="Pick a move to review">
                  Use the board controls, the move list, or the critical-moment list.
                </EmptyState>
              )}

              {hasNextCritical ? (
                <div className="critical-list">
                  <div className="critical-list__head">
                    <p className="ef-label">Your critical moments</p>
                    <span className="ef-mono">{criticalMoments.length}</span>
                  </div>
                  <ol>
                    {criticalMoments.map((move) => (
                      <li key={move.id}>
                        <button
                          type="button"
                          className={`critical-row${move.ply === selectedPly ? ' is-active' : ''}`}
                          onClick={() => {
                            setMode('review')
                            selectPly(move.ply)
                          }}
                        >
                          <span className="ef-mono critical-row__move">
                            {move.move_number}. {move.san}
                          </span>
                          <span className={`critical-row__class ${classificationClass(move.classification)}`}>
                            {classificationLabel(move.classification)}
                          </span>
                          <span className="ef-mono critical-row__epl">
                            −{formatPercent((move.epl ?? 0) * 100, 1)} win
                          </span>
                        </button>
                      </li>
                    ))}
                  </ol>
                </div>
              ) : null}
            </div>
          ) : null}

          {mode === 'moves' ? (
            <div className="moves-panel">
              <div className="section-head">
                <div><p className="ef-label">Full game</p><h2>Every move, in order</h2></div>
                <span className="ef-mono">{moves.length} plies</span>
              </div>
              <div className="move-list" ref={listRef} tabIndex={0} aria-label="Moves">
                {moves.map((move) => (
                  <button
                    key={move.id}
                    type="button"
                    data-ply={move.ply}
                    aria-current={move.ply === selectedPly ? 'true' : undefined}
                    className={`move-row${move.ply === selectedPly ? ' is-selected' : ''} ${classificationClass(move.classification)}`}
                    onClick={() => selectPly(move.ply)}
                  >
                    <span className="move-row__num ef-mono">{move.move_number}</span>
                    <span className="move-row__san ef-mono">{move.san}</span>
                    <span className="move-row__eval ef-mono">
                      {formatEval(move.eval_after_cp, move.eval_mate)}
                    </span>
                    <span className="move-row__class">{classificationLabel(move.classification)}</span>
                  </button>
                ))}
              </div>
            </div>
          ) : null}

          {mode === 'report' ? (
            <div className="report-panel">
              <div className="match-score">
                <div>
                  <p className="ef-label">Your accuracy</p>
                  <p className="match-score__number ef-mono">
                    {formatPercent(game.game.accuracy_user, 1)}
                  </p>
                </div>
                <div className="match-score__phase">
                  {phases.map((phase) => (
                    <div key={phase.key} className="phase-row">
                      <span>{phase.label}</span>
                      <div className="phase-bar"><span style={{ width: `${phase.value ?? 0}%` }} /></div>
                      <span className="ef-mono">{formatPercent(phase.value, 0)}</span>
                    </div>
                  ))}
                </div>
              </div>

              {openingViolations.length > 0 ? (
                <div className="report-block">
                  <p className="ef-label">Opening principle flags</p>
                  <ul className="report-list">
                    {openingViolations.map((violation) => (
                      <li key={`${violation.code}-${violation.ply}`}>
                        <button type="button" onClick={() => selectPly(violation.ply)}>
                          <span className="ef-mono">ply {violation.ply}</span>
                          <strong>{violation.label}</strong>
                          <span>{violation.detail}</span>
                        </button>
                      </li>
                    ))}
                  </ul>
                </div>
              ) : null}

              <div className="report-block">
                <p className="ef-label">Game tags</p>
                <div className="match-tags">
                  {(analysis?.game_tags ?? []).length > 0
                    ? analysis?.game_tags?.map((tag) => <Tag key={tag} tone="sienna">{tagLabel(tag)}</Tag>)
                    : <span className="report-empty">No game-level tags.</span>}
                </div>
              </div>

              <div className="report-block">
                <p className="ef-label">Your reflection thread</p>
                {game.reflections.length === 0 ? (
                  <p className="report-empty">No reflections yet. Open a critical moment and write one sentence about what you saw.</p>
                ) : (
                  <ol className="reflection-list reflection-list--compact">
                    {game.reflections.map((reflection) => {
                      const move = moves.find((item) => item.id === reflection.move_id)
                      return (
                        <li key={reflection.id}>
                          <div className="reflection-list__head">
                            <span className="ef-mono">{move ? `${move.move_number}. ${move.san}` : 'move'}</span>
                            <span className="ef-mono reflection-list__status">{reflection.ai_status}</span>
                          </div>
                          <p className="reflection-list__quote">“{reflection.explanation}”</p>
                          {reflection.next_time_rule ? <p className="reflection-list__rule">{reflection.next_time_rule}</p> : null}
                        </li>
                      )
                    })}
                  </ol>
                )}
              </div>
            </div>
          ) : null}
        </section>
      </div>

      {mode !== 'review' ? (
        <section className="match-chart">
          <EvalChart moves={moves} userColor={userColor} selectedPly={selectedPly} onSelect={selectPly} />
        </section>
      ) : null}

      <p className="match-nav">
        <Link to="/games">← Back to games</Link>
        <span className="ef-mono">
          {game.game.white_username} vs {opponent} · {formatDate(game.game.start_time)}
        </span>
      </p>

      <ReflectDrawer
        key={`${selectedMove?.id ?? 'none'}-${reflectOpen}`}
        open={reflectOpen}
        gameId={game.game.id}
        move={selectedMove}
        onClose={() => setReflectOpen(false)}
        onSaved={() => void queryClient.invalidateQueries({ queryKey: ['game', id] })}
      />
    </>
  )
}
