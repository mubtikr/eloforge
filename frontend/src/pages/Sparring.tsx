import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Chess } from 'chess.js'
import { ArrowRight, Lightbulb, RotateCcw, Swords } from 'lucide-react'
import { useMemo, useState, type FormEvent } from 'react'
import { Link } from 'react-router-dom'

import { api, getErrorMessage } from '../api/client'
import type {
  SparringHint,
  SparringMoveResponse,
  SparringSessionDto,
} from '../api/types'
import Board, { type BoardArrow } from '../components/Board'
import { EmptyState, ErrorBlock, LoadingBlock, PageHeader, Tag } from '../components/ui'
import { formatPercent, tagLabel } from '../lib/format'

function parseUci(value: string): { from: string; to: string; uci: string } | null {
  const match = /^([a-h][1-8])([a-h][1-8])([qrbn])?$/i.exec(value.trim())
  if (!match) return null
  const from = match[1]
  const to = match[2]
  if (!from || !to) return null
  const promotion = match[3]?.toLowerCase()
  return { from, to, uci: `${from}${to}${promotion ?? ''}` }
}

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

function statusClass(status: string): string {
  if (status === 'won') return 'win'
  if (status === 'lost') return 'loss'
  return 'draw'
}

export default function Sparring() {
  const queryClient = useQueryClient()
  const [sessionOverride, setSession] = useState<SparringSessionDto | null>(null)
  const [message, setMessage] = useState<string | null>(null)
  const [uciInput, setUciInput] = useState('')
  const [pendingFen, setPendingFen] = useState<string | null>(null)
  const [pendingLastMove, setPendingLastMove] = useState<string | null>(null)
  const [thinking, setThinking] = useState(false)
  const [hint, setHint] = useState<SparringHint | null>(null)

  const sessions = useQuery({
    queryKey: ['sparring'],
    queryFn: () => api<SparringSessionDto[]>('/api/sparring'),
  })

  const session =
    sessionOverride ??
    sessions.data?.find((item) => item.status === 'active') ??
    sessions.data?.[0] ??
    null

  const start = useMutation({
    mutationFn: () =>
      api<SparringSessionDto>('/api/sparring/start', {
        method: 'POST',
        body: JSON.stringify({}),
      }),
    onSuccess: (data) => {
      setSession(data)
      setMessage(null)
      setPendingFen(null)
      setPendingLastMove(null)
      setThinking(false)
      setHint(null)
      void queryClient.invalidateQueries({ queryKey: ['sparring'] })
    },
  })

  const move = useMutation({
    mutationFn: (uci: string) =>
      api<SparringMoveResponse>(`/api/sparring/${session?.id}/move`, {
        method: 'POST',
        body: JSON.stringify({ uci }),
      }),
    onSuccess: (data) => {
      setSession(data.session)
      setMessage(data.message)
      setPendingFen(null)
      setPendingLastMove(null)
      setThinking(false)
      setHint(null)
      void queryClient.invalidateQueries({ queryKey: ['sparring'] })
      void queryClient.invalidateQueries({ queryKey: ['puzzles'] })
      void queryClient.invalidateQueries({ queryKey: ['roadmap'] })
    },
    onError: (error) => {
      setPendingFen(null)
      setPendingLastMove(null)
      setThinking(false)
      setMessage(getErrorMessage(error))
    },
  })

  const hintMutation = useMutation({
    mutationFn: () => api<SparringHint>(`/api/sparring/${session?.id}/hint`),
    onSuccess: (data) => {
      setHint(data)
      setMessage(data.message)
    },
    onError: (error) => setMessage(getErrorMessage(error)),
  })

  const displayFen = pendingFen ?? session?.fen ?? ''
  const dests = useMemo(() => destsFor(displayFen), [displayFen])
  const userColor = session?.user_color === 'black' ? 'black' : 'white'
  const lastMove = pendingLastMove ?? session?.moves.at(-1)?.uci ?? null
  const canMove = Boolean(session && session.status === 'active' && !thinking && !pendingFen)

  function playMove(from: string, to: string, promotion = 'q') {
    if (!session || !canMove) return
    let uci = `${from}${to}`
    let after: string
    try {
      const chess = new Chess(session.fen)
      const played = chess.move({ from, to, promotion })
      if (!played) return
      uci = `${played.from}${played.to}${played.promotion ?? ''}`
      after = chess.fen()
    } catch {
      return
    }
    setPendingFen(after)
    setPendingLastMove(uci)
    setThinking(true)
    setMessage('Stockfish is thinking…')
    setHint(null)
    move.mutate(uci)
  }

  function submitKeyboard(event: FormEvent) {
    event.preventDefault()
    if (!session || !canMove) return
    const value = uciInput.trim()
    if (!value) return
    try {
      const chess = new Chess(displayFen)
      const played = chess.move(value)
      if (played) {
        playMove(played.from, played.to, played.promotion ?? 'q')
        setUciInput('')
        return
      }
    } catch {
      // fall through to UCI parsing for engines/clients that send square notation
    }
    const parsed = parseUci(value)
    if (!parsed) return
    playMove(parsed.from, parsed.to, parsed.uci[4] ?? 'q')
    setUciInput('')
  }

  if (sessions.isLoading) return <LoadingBlock label="Opening the Endgame Arena…" />

  return (
    <>
      <PageHeader
        className="game-page sparring-page"
        eyebrow="Module E · Endgame sparring"
        title="Finish what you started."
        description="Replay the exact endgame you failed to convert. You play your colour; a rating-calibrated Stockfish defends."
        actions={
          <button className="ef-btn ef-btn--primary" type="button" onClick={() => start.mutate()} disabled={start.isPending}>
            <Swords size={15} />
            {start.isPending ? 'Preparing…' : 'Start next endgame'}
          </button>
        }
      />

      {start.error ? <p className="form-error">{getErrorMessage(start.error)}</p> : null}
      {sessions.isError ? <ErrorBlock message={getErrorMessage(sessions.error)} retry={() => void sessions.refetch()} /> : null}

      {!session ? (
        <EmptyState title="No endgame drill queued" action={<Link className="ef-btn ef-btn--ghost" to="/puzzles">Open puzzles</Link>}>
          When the analyzer tags a thrown winning endgame, that exact position becomes a sparring session. If you have none yet, analyse a game you lost from a winning position.
        </EmptyState>
      ) : (
        <div className="sparring-layout">
          <section className="sparring-board">
            <div className="arena-objective">
              <div>
                <p className="ef-label">Your objective</p>
                <p>
                  {session.white_username && session.black_username
                    ? `Convert ${session.white_username} vs ${session.black_username} as ${session.user_color}.`
                    : `Convert this endgame as ${session.user_color}.`}
                </p>
              </div>
              <span className="ef-mono">
                {session.puzzle_epl !== null ? `${formatPercent(session.puzzle_epl * 100, 1)} win lost` : 'winning endgame'}
              </span>
            </div>
            <Board
              fen={displayFen}
              orientation={userColor}
              interactive={canMove}
              movableColor={userColor}
              dests={dests}
              onMove={(from, to) => playMove(from, to)}
              lastMove={lastMove ? [lastMove.slice(0, 2), lastMove.slice(2, 4)] : null}
              arrows={
                hint
                  ? [{ orig: hint.uci.slice(0, 2), dest: hint.uci.slice(2, 4), kind: 'best' } as BoardArrow]
                  : []
              }
              ariaLabel="Endgame sparring board"
            />
            <div className="board-controls">
              <span className={`result-word result-word--${statusClass(session.status)}`}>
                {session.status === 'active' ? (thinking ? 'Stockfish thinking' : 'Your move') : session.status}
              </span>
              <span className="ef-mono">vs Stockfish {session.enemy_elo ?? '—'}</span>
              <span className="board-controls__spacer" />
              <button className="ef-btn ef-btn--ghost" type="button" onClick={() => start.mutate()} disabled={start.isPending}>
                <RotateCcw size={14} /> New drill
              </button>
            </div>
          </section>

          <aside className="sparring-panel">
            <div className="sparring-status" aria-live="polite">
              <p className="ef-label">Arena status</p>
              <p className="sparring-status__message">
                {message ??
                  (session.status === 'active'
                    ? thinking
                      ? 'Stockfish is calculating its reply.'
                      : 'Your move. Improve the king or rook before touching a pawn.'
                    : session.status === 'won'
                      ? 'Mastered. The drill is marked complete.'
                      : 'Drill finished. Replay or start another.')}
              </p>
              {move.error ? <p className="form-error">{getErrorMessage(move.error)}</p> : null}
              <div className="sparring-facts">
                <div><span className="ef-label">Your colour</span><span>{session.user_color}</span></div>
                <div><span className="ef-label">Moves</span><span className="ef-mono">{session.moves.length}</span></div>
                <div><span className="ef-label">Drill</span><span>{session.puzzle_id ? 'personal endgame' : 'arena'}</span></div>
              </div>
              {session.puzzle_tags.length > 0 ? (
                <div className="match-tags">
                  {session.puzzle_tags.map((tag) => <Tag key={tag} tone="sienna">{tagLabel(tag)}</Tag>)}
                </div>
              ) : null}
              <div className="sparring-actions">
                <button
                  className="ef-btn ef-btn--ghost"
                  type="button"
                  onClick={() => hintMutation.mutate()}
                  disabled={!canMove || hintMutation.isPending}
                >
                  <Lightbulb size={14} /> {hintMutation.isPending ? 'Thinking…' : 'Coach hint'}
                </button>
                {session.game_id ? (
                  <Link className="ef-btn ef-btn--ghost" to={`/games/${session.game_id}`}>
                    Source game <ArrowRight size={14} />
                  </Link>
                ) : null}
              </div>
            </div>

            <div className="sparring-moves" tabIndex={0} aria-label="Sparring move record">
              <p className="ef-label">Move record</p>
              {session.moves.length === 0 ? (
                <p className="sparring-moves__empty">No moves yet. Start with a king or rook improvement.</p>
              ) : (
                <ol>
                  {session.moves.map((ply, index) => (
                    <li key={`${ply.uci}-${index}`}>
                      <span className="ef-mono">{ply.move_number}.</span>
                      <span className={`ef-mono sparring-moves__${ply.color}`}>{ply.san}</span>
                      <span className="ef-mono">{ply.color === userColor ? 'you' : 'engine'}</span>
                    </li>
                  ))}
                </ol>
              )}
            </div>

            <form className="kbd-move" onSubmit={submitKeyboard}>
              <label>
                <span className="ef-label">Keyboard move</span>
                <input
                  className="ef-input kbd-move__input"
                  value={uciInput}
                  onChange={(event) => setUciInput(event.target.value)}
                  placeholder="Nf6 or e7e5"
                  maxLength={8}
                  disabled={!canMove}
                  spellCheck={false}
                />
              </label>
              <button className="ef-btn ef-btn--ghost" type="submit" disabled={!canMove || !uciInput.trim()}>
                Play move
              </button>
            </form>

            <div className="sparring-rule">
              <p className="ef-label">Conversion rule</p>
              <p>
                In a winning endgame, improve the king or rook before pushing pawns. Check that the opponent has a legal
                move before every quiet move.
              </p>
              {hint ? <p className="sparring-hint">Hint: {hint.san ?? hint.uci}. Use it as a plan; the drill still has to be converted.</p> : null}
              {session.status === 'won' ? <Tag tone="good">drill mastered</Tag> : null}
            </div>

            <Link className="ef-btn ef-btn--ghost" to="/puzzles">
              Back to puzzles <ArrowRight size={14} />
            </Link>
          </aside>
        </div>
      )}

      {sessions.data && sessions.data.length > 0 ? (
        <section className="sparring-history">
          <div className="section-head">
            <div><p className="ef-label">Arena history</p><h2>Recent endgame attempts</h2></div>
          </div>
          <div className="ledger">
            {sessions.data.slice(0, 8).map((item) => (
              <button
                key={item.id}
                className="ledger__row"
                type="button"
                onClick={() => {
                  setSession(item)
                  setPendingFen(null)
                  setPendingLastMove(null)
                  setThinking(false)
                  setMessage(null)
                  setHint(null)
                }}
              >
                <span className={`result-mark result-mark--${statusClass(item.status)}`}>
                  {item.status === 'won' ? 'W' : item.status === 'lost' ? 'L' : '–'}
                </span>
                <span className="ledger__opponent">Stockfish {item.enemy_elo ?? ''}</span>
                <span className="ef-mono">{item.moves.length} plies</span>
                <span>{item.status}</span>
              </button>
            ))}
          </div>
        </section>
      ) : null}
    </>
  )
}
