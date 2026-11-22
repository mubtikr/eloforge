import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Chess } from 'chess.js'
import { ChevronLeft, ChevronRight, Download, MessageSquareQuote, Sparkles } from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Link, useParams } from 'react-router-dom'

import { api, getErrorMessage } from '../api/client'
import type { AnalyzeResponse, GameDetail, MoveDto } from '../api/types'
import Board, { type BoardArrow } from '../components/Board'
import EvalChart from '../components/EvalChart'
import ReflectDrawer from '../components/ReflectDrawer'
import { EmptyState, ErrorBlock, LoadingBlock, Tag } from '../components/ui'
import {
  classificationClass,
  classificationLabel,
  formatClock,
  formatDate,
  formatEval,
  formatPercent,
  tagLabel,
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

function isBad(move: MoveDto): boolean {
  return ['inaccuracy', 'mistake', 'blunder', 'missed_win'].includes(move.classification ?? '')
}

export default function MatchLab() {
  const { id } = useParams<{ id: string }>()
  const queryClient = useQueryClient()
  const [selection, setSelection] = useState<{ gameId: string | undefined; ply: number }>({
    gameId: id,
    ply: 1,
  })
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
  const selectedPly = selection.gameId === id ? selection.ply : 1
  const setSelectedPly = useCallback(
    (next: number | ((current: number) => number)) => {
      setSelection((previous) => {
        const base = previous.gameId === id ? previous.ply : 1
        const value = typeof next === 'function' ? next(base) : next
        return { gameId: id, ply: value }
      })
    },
    [id],
  )
  const selectedMove: MoveDto | null = moves.find((move) => move.ply === selectedPly) ?? null
  const previousMove: MoveDto | null = moves.find((move) => move.ply === selectedPly - 1) ?? null
  const userColor = game?.game.user_color === 'black' ? 'black' : 'white'
  const analysis = (game?.analysis ?? null) as AnalysisShape | null
  const accuracy = analysis?.accuracy

  useEffect(() => {
    if (!game) return
    const handler = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null
      if (target && ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName)) return
      if (reflectOpen) return
      if (event.key === 'ArrowLeft') {
        event.preventDefault()
        setSelectedPly((current) => Math.max(1, current - 1))
      } else if (event.key === 'ArrowRight') {
        event.preventDefault()
        setSelectedPly((current) => Math.min(moves.length, current + 1))
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [game, moves.length, reflectOpen, setSelectedPly])

  useEffect(() => {
    const node = listRef.current?.querySelector<HTMLElement>(`[data-ply="${selectedPly}"]`)
    node?.scrollIntoView({ block: 'nearest' })
  }, [selectedPly])

  const check = useMemo(() => {
    const fen = selectedMove?.fen_before ?? game?.initial_fen
    if (!fen) return false
    try {
      return new Chess(fen).isCheck()
    } catch {
      return false
    }
  }, [selectedMove, game?.initial_fen])

  const arrows: BoardArrow[] = useMemo(() => {
    if (!selectedMove) return []
    const list: BoardArrow[] = []
    if (selectedMove.best_move_uci) {
      list.push({
        orig: selectedMove.best_move_uci.slice(0, 2),
        dest: selectedMove.best_move_uci.slice(2, 4),
        kind: 'best',
      })
    }
    if (isBad(selectedMove) && selectedMove.uci) {
      list.push({
        orig: selectedMove.uci.slice(0, 2),
        dest: selectedMove.uci.slice(2, 4),
        kind: 'played',
      })
    }
    return list
  }, [selectedMove])

  const lastMove = useMemo(() => {
    const source = previousMove ?? null
    if (!source) return null
    return [source.uci.slice(0, 2), source.uci.slice(2, 4)]
  }, [previousMove])

  if (gameQuery.isLoading) return <LoadingBlock label="Loading Match Lab…" />
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

  return (
    <>
      <header className="match-head">
        <div>
          <p className="ef-label">
            {game.game.time_class ?? 'chess'} · {formatDate(game.game.start_time)}
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
            <span>{game.game.opening_name ?? game.game.opening_eco ?? 'Unknown opening'}</span>
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
            The board is available; classifications and reflections appear when Stockfish finishes.
            {game.game.analysis_status === 'failed' ? ' The last attempt failed; retry above.' : ''}
          </p>
        </div>
      ) : null}

      <div className="match-layout">
        <section className="match-board">
          <Board
            fen={selectedMove?.fen_before ?? game.initial_fen}
            orientation={userColor}
            lastMove={lastMove}
            arrows={arrows}
            check={check}
            ariaLabel={`Position before ${selectedMove ? `${selectedMove.move_number}. ${selectedMove.san}` : 'the first move'}`}
          />
          <div className="board-controls">
            <button
              className="icon-btn"
              type="button"
              onClick={() => setSelectedPly((current) => Math.max(1, current - 1))}
              aria-label="Previous move"
            >
              <ChevronLeft size={17} />
            </button>
            <span className="ef-mono board-controls__label">
              {selectedMove ? `${selectedMove.move_number}. ${selectedMove.san}` : 'Start'}
            </span>
            <button
              className="icon-btn"
              type="button"
              onClick={() => setSelectedPly((current) => Math.min(moves.length, current + 1))}
              aria-label="Next move"
            >
              <ChevronRight size={17} />
            </button>
            <span className="board-controls__spacer" />
            <span className="ef-mono board-controls__clock">
              {formatClock(selectedMove?.clock_remaining_sec)}
            </span>
          </div>
          <div className="match-evidence">
            <span><i className="swatch swatch--best" /> Stockfish best</span>
            <span><i className="swatch swatch--played" /> Your move</span>
            <span className="ef-mono">depth 14 → 20</span>
          </div>
        </section>

        <section className="match-panel">
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

          <div className="move-list" ref={listRef} role="listbox" aria-label="Moves">
            {moves.map((move) => (
              <button
                key={move.id}
                type="button"
                data-ply={move.ply}
                role="option"
                aria-selected={move.ply === selectedPly}
                className={`move-row${move.ply === selectedPly ? ' is-selected' : ''} ${classificationClass(move.classification)}`}
                onClick={() => setSelectedPly(move.ply)}
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

          <div className="match-detail">
            <div className="match-detail__head">
              <div>
                <p className="ef-label">Selected move</p>
                <p className="match-detail__san ef-mono">
                  {selectedMove ? `${selectedMove.move_number}. ${selectedMove.san}` : 'Start position'}
                </p>
              </div>
              {selectedMove?.classification ? (
                <span className={`class-pill is-${selectedMove.classification}`}>
                  {classificationLabel(selectedMove.classification)}
                </span>
              ) : null}
            </div>
            {selectedMove ? (
              <>
                <dl className="move-facts">
                  <div><dt>Eval before</dt><dd className="ef-mono">{formatEval(selectedMove.eval_before_cp, null)}</dd></div>
                  <div><dt>Eval after</dt><dd className="ef-mono">{formatEval(selectedMove.eval_after_cp, selectedMove.eval_mate)}</dd></div>
                  <div><dt>Win lost</dt><dd className="ef-mono">{formatPercent((selectedMove.epl ?? 0) * 100, 1)}</dd></div>
                  <div><dt>Time spent</dt><dd className="ef-mono">{selectedMove.time_spent_sec?.toFixed(1) ?? '—'}s</dd></div>
                  <div><dt>Best move</dt><dd className="ef-mono">{selectedMove.best_move_san ?? '—'}</dd></div>
                </dl>
                {moveTags.length > 0 ? (
                  <div className="match-tags">
                    {moveTags.map((tag) => <Tag key={tag} tone="sienna">{tagLabel(tag)}</Tag>)}
                  </div>
                ) : null}
                {isBad(selectedMove) && selectedMove.color === game.game.user_color && game.game.analysis_status === 'done' ? (
                  <button className="ef-btn ef-btn--primary match-reflect" type="button" onClick={() => setReflectOpen(true)}>
                    <MessageSquareQuote size={15} />
                    Why did I play this?
                  </button>
                ) : null}
              </>
            ) : (
              <p className="match-detail__hint">Select a move to see the engine facts and the coach prompt.</p>
            )}
          </div>
        </section>
      </div>

      <section className="match-chart">
        <EvalChart moves={moves} userColor={userColor} selectedPly={selectedPly} onSelect={setSelectedPly} />
      </section>

      {openingViolations.length > 0 ? (
        <section className="violations">
          <p className="ef-label">Opening principle flags</p>
          <ul>
            {openingViolations.map((violation) => (
              <li key={`${violation.code}-${violation.ply}`}>
                <button type="button" className="violations__link" onClick={() => setSelectedPly(violation.ply)}>
                  <span className="ef-mono">ply {violation.ply}</span>
                  <strong>{violation.label}</strong>
                  <span>{violation.detail}</span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      <section className="reflections">
        <div className="section-head">
          <div>
            <p className="ef-label">Reflection thread</p>
            <h2>What you said you were thinking</h2>
          </div>
          <span className="ef-mono">{game.reflections.length} note(s)</span>
        </div>
        {game.reflections.length === 0 ? (
          <EmptyState title="No reflections yet">
            Open a mistake, press “Why did I play this?”, and write one honest sentence. The coach will turn it into a next-time rule.
          </EmptyState>
        ) : (
          <ol className="reflection-list">
            {game.reflections.map((reflection) => {
              const move = moves.find((item) => item.id === reflection.move_id)
              return (
                <li key={reflection.id}>
                  <div className="reflection-list__head">
                    <span className="ef-mono">
                      {move ? `${move.move_number}. ${move.san}` : 'move'}
                    </span>
                    <span className="ef-mono reflection-list__status">{reflection.ai_status}</span>
                  </div>
                  <p className="reflection-list__quote">“{reflection.explanation}”</p>
                  {reflection.next_time_rule ? (
                    <p className="reflection-list__rule">{reflection.next_time_rule}</p>
                  ) : null}
                  {reflection.reason_tags.length > 0 ? (
                    <div className="match-tags">
                      {reflection.reason_tags.map((tag) => <Tag key={tag}>{tagLabel(tag)}</Tag>)}
                    </div>
                  ) : null}
                </li>
              )
            })}
          </ol>
        )}
      </section>

      <p className="match-nav">
        <Link to="/app">← Back to Today</Link>
        <span className="ef-mono">
          {game.game.white_username} vs {opponent}
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
