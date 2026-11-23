import { Chess } from 'chess.js'
import { Chessground } from 'chessground'
import type { Api } from 'chessground/api'
import type { Config } from 'chessground/config'
import type { Color, Dests, Key } from 'chessground/types'
import { useEffect, useRef } from 'react'

export interface BoardArrow {
  orig: string
  dest: string
  kind: 'best' | 'played'
}

interface BoardProps {
  fen: string
  orientation: 'white' | 'black'
  lastMove?: string[] | null
  arrows?: BoardArrow[]
  selectedSquare?: string | null
  check?: boolean
  interactive?: boolean
  movableColor?: 'white' | 'black'
  dests?: Map<string, string[]>
  onMove?: (from: string, to: string) => void
  ariaLabel?: string
}

const BRUSHES = {
  green: { key: 'green', color: '#b98a3e', opacity: 0.9, lineWidth: 11 },
  red: { key: 'red', color: '#a64a24', opacity: 0.82, lineWidth: 9 },
  blue: { key: 'blue', color: '#3f5560', opacity: 0.8, lineWidth: 9 },
  yellow: { key: 'yellow', color: '#d7b56d', opacity: 0.9, lineWidth: 9 },
  brass: { key: 'brass', color: '#b98a3e', opacity: 0.9, lineWidth: 11 },
  sienna: { key: 'sienna', color: '#a64a24', opacity: 0.82, lineWidth: 9 },
}

function boardSignature(fen: string): string {
  return fen.split(' ').slice(0, 4).join(' ')
}

function singleStep(previous: string, next: string): { from: string; to: string } | null {
  if (boardSignature(previous) === boardSignature(next)) return null
  try {
    const before = new Chess(previous)
    const target = boardSignature(next)
    for (const move of before.moves({ verbose: true })) {
      if (move.san.startsWith('O-O') || move.promotion) continue
      const candidate = new Chess(previous)
      candidate.move(move.san)
      if (boardSignature(candidate.fen()) === target) {
        return { from: move.from, to: move.to }
      }
    }
  } catch {
    return null
  }
  return null
}

export default function Board({
  fen,
  orientation,
  lastMove,
  arrows = [],
  selectedSquare = null,
  check = false,
  interactive = false,
  movableColor,
  dests,
  onMove,
  ariaLabel = 'Chess position',
}: BoardProps) {
  const hostRef = useRef<HTMLDivElement>(null)
  const apiRef = useRef<Api | null>(null)
  const moveRef = useRef(onMove)
  const renderedFenRef = useRef<string | null>(null)
  moveRef.current = onMove

  useEffect(() => {
    if (!hostRef.current) return
    const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    const config: Config = {
      fen,
      orientation,
      coordinates: true,
      blockTouchScroll: true,
      animation: { enabled: !reduce, duration: 200 },
      highlight: { lastMove: true, check: true },
      drawable: {
        enabled: true,
        brushes: BRUSHES,
        autoShapes: arrows.map((arrow) => ({
          orig: arrow.orig as Key,
          dest: arrow.dest as Key,
          brush: arrow.kind === 'best' ? 'brass' : 'sienna',
        })),
      },
      viewOnly: !interactive,
    }
    if (interactive) {
      config.movable = {
        free: false,
        color: movableColor ?? orientation,
        dests: (dests ?? new Map()) as Dests,
        showDests: true,
        events: {
          after: (orig, dest) => {
            moveRef.current?.(orig, dest)
          },
        },
      }
    }
    const api = Chessground(hostRef.current, config)
    apiRef.current = api
    renderedFenRef.current = fen
    return () => {
      api.destroy()
      apiRef.current = null
      renderedFenRef.current = null
    }
    // The board is created once; updates flow through the effect below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  useEffect(() => {
    const api = apiRef.current
    if (!api) return

    const update: Config = {
      orientation: orientation as Color,
      turnColor: fen.split(' ')[1] === 'b' ? 'black' : 'white',
      check,
      lastMove: (lastMove ?? undefined) as Key[] | undefined,
      selected: (selectedSquare ?? undefined) as Key | undefined,
      drawable: {
        brushes: BRUSHES,
        autoShapes: arrows.map((arrow) => ({
          orig: arrow.orig as Key,
          dest: arrow.dest as Key,
          brush: arrow.kind === 'best' ? 'brass' : 'sienna',
        })),
      },
      viewOnly: !interactive,
    }
    if (interactive) {
      update.movable = {
        free: false,
        color: movableColor ?? orientation,
        dests: (dests ?? new Map()) as Dests,
        showDests: true,
        events: {
          after: (orig, dest) => {
            moveRef.current?.(orig, dest)
          },
        },
      }
    }

    const previous = renderedFenRef.current
    const step = previous ? singleStep(previous, fen) : null
    if (previous && previous !== fen && step) {
      // Animate exactly one arriving move and update metadata without re-reading
      // the whole FEN, which is what caused the piece to jump back.
      api.move(step.from as Key, step.to as Key)
      api.set(update)
    } else {
      api.set({ ...update, fen })
    }
    renderedFenRef.current = fen
  }, [fen, orientation, lastMove, arrows, selectedSquare, check, interactive, movableColor, dests])

  return (
    <div
      className="board-frame"
      data-fen={fen}
      role={interactive ? 'group' : 'img'}
      aria-label={ariaLabel}
      aria-roledescription={interactive ? 'interactive chess board' : 'chess position'}
    >
      <div className="board-host" ref={hostRef} />
      {interactive ? (
        <p className="sr-only">
          Use the move field below the board for keyboard and screen-reader move entry.
          The move list and coach notes describe the current position.
        </p>
      ) : null}
    </div>
  )
}
