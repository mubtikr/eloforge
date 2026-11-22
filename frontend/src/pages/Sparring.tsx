import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Chess } from 'chess.js'
import { ArrowRight, RotateCcw, Swords } from 'lucide-react'
import { useMemo, useState, type FormEvent } from 'react'
import { Link } from 'react-router-dom'

import { api, getErrorMessage } from '../api/client'
import type { SparringMoveResponse, SparringSessionDto } from '../api/types'
import Board from '../components/Board'
import { EmptyState, ErrorBlock, LoadingBlock, PageHeader, Tag } from '../components/ui'

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

export default function Sparring() {
  const queryClient = useQueryClient()
  const [sessionOverride, setSession] = useState<SparringSessionDto | null>(null)
  const [message, setMessage] = useState<string | null>(null)
  const [uciInput, setUciInput] = useState('')

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
      void queryClient.invalidateQueries({ queryKey: ['sparring'] })
      void queryClient.invalidateQueries({ queryKey: ['puzzles'] })
      void queryClient.invalidateQueries({ queryKey: ['roadmap'] })
    },
  })

  const dests = useMemo(() => (session ? destsFor(session.fen) : new Map()), [session])
  const userColor = session?.user_color === 'black' ? 'black' : 'white'
  const lastMove = session?.moves.at(-1)?.uci

  function playMove(from: string, to: string, promotion = 'q') {
    if (!session || session.status !== 'active' || move.isPending) return
    let uci = `${from}${to}`
    try {
      const chess = new Chess(session.fen)
      const played = chess.move({ from, to, promotion })
      if (!played) return
      uci = `${played.from}${played.to}${played.promotion ?? ''}`
    } catch {
      return
    }
    move.mutate(uci)
  }

  function handleMove(from: string, to: string) {
    playMove(from, to)
  }

  function submitKeyboard(event: FormEvent) {
    event.preventDefault()
    const parsed = parseUci(uciInput)
    if (!parsed) return
    playMove(parsed.from, parsed.to, parsed.uci[4] ?? 'q')
    setUciInput('')
  }

  if (sessions.isLoading) return <LoadingBlock label="Opening the Endgame Arena…" />

  return (
    <>
      <PageHeader
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
            <Board
              fen={session.fen}
              orientation={userColor}
              interactive={session.status === 'active' && !move.isPending}
              movableColor={userColor}
              dests={dests}
              onMove={handleMove}
              lastMove={lastMove ? [lastMove.slice(0, 2), lastMove.slice(2, 4)] : null}
              ariaLabel="Endgame sparring board"
            />
            <div className="board-controls">
              <span className={`result-word result-word--${session.status === 'won' ? 'win' : session.status === 'lost' ? 'loss' : 'draw'}`}>
                {session.status}
              </span>
              <span className="ef-mono">vs Stockfish {session.enemy_elo ?? '—'}</span>
              <span className="board-controls__spacer" />
              <button className="ef-btn ef-btn--ghost" type="button" onClick={() => start.mutate()} disabled={start.isPending}>
                <RotateCcw size={14} /> New drill
              </button>
            </div>
          </section>

          <aside className="sparring-panel">
            <div className="sparring-status">
              <p className="ef-label">Arena status</p>
              <p className="sparring-status__message">
                {message ?? (session.status === 'active' ? 'Your move. Convert the position cleanly.' : session.status === 'won' ? 'Mastered. The puzzle is marked complete.' : 'Drill finished. Replay or start another.')}
              </p>
              {move.error ? <p className="form-error">{getErrorMessage(move.error)}</p> : null}
              <div className="sparring-facts">
                <div><span className="ef-label">Your colour</span><span>{session.user_color}</span></div>
                <div><span className="ef-label">Moves</span><span className="ef-mono">{session.moves.length}</span></div>
                <div><span className="ef-label">Drill</span><span>{session.puzzle_id ? 'personal endgame' : 'arena'}</span></div>
              </div>
            </div>

            <div className="sparring-moves">
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
                  placeholder="e7e5"
                  maxLength={5}
                  disabled={session.status !== 'active' || move.isPending}
                  spellCheck={false}
                />
              </label>
              <button
                className="ef-btn ef-btn--ghost"
                type="submit"
                disabled={session.status !== 'active' || move.isPending || !uciInput.trim()}
              >
                Play move
              </button>
            </form>

            <div className="sparring-rule">
              <p className="ef-label">Conversion rule</p>
              <p>
                In a winning endgame, improve the king or rook before pushing pawns. Check that the opponent has a legal move before every quiet move.
              </p>
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
              <button key={item.id} className="ledger__row" type="button" onClick={() => setSession(item)}>
                <span className={`result-mark result-mark--${item.status === 'won' ? 'win' : item.status === 'lost' ? 'loss' : 'draw'}`}>
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
