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

export default function Board({
  fen,
  orientation,
  lastMove,
  arrows = [],
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
  moveRef.current = onMove

  useEffect(() => {
    if (!hostRef.current) return
    const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    const config: Config = {
      fen,
      orientation,
      coordinates: true,
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
    return () => {
      api.destroy()
      apiRef.current = null
    }
    // The board is created once; updates flow through the effect below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  useEffect(() => {
    const api = apiRef.current
    if (!api) return
    const update: Config = {
      fen,
      orientation: orientation as Color,
      turnColor: fen.split(' ')[1] === 'b' ? 'black' : 'white',
      check,
      lastMove: (lastMove ?? undefined) as Key[] | undefined,
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
    api.set(update)
  }, [fen, orientation, lastMove, arrows, check, interactive, movableColor, dests])

  return (
    <div
      className="board-frame"
      role="img"
      aria-label={ariaLabel}
      ref={(node) => {
        // keep a stable host element while allowing the ref callback
        hostRef.current = node
      }}
    >
      <div className="board-host" ref={hostRef} />
    </div>
  )
}
