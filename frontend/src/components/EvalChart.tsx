import { useCallback, useMemo, useRef } from 'react'

import type { MoveDto } from '../api/types'
import { formatEval } from '../lib/format'

interface EvalChartProps {
  moves: MoveDto[]
  userColor: 'white' | 'black'
  selectedPly: number
  onSelect: (ply: number) => void
}

const WIDTH = 820
const HEIGHT = 216
const PAD = { top: 18, right: 12, bottom: 24, left: 34 }

export default function EvalChart({ moves, userColor, selectedPly, onSelect }: EvalChartProps) {
  const frame = useRef<number | null>(null)
  const pending = useRef<number | null>(null)
  const svgRef = useRef<SVGSVGElement>(null)

  const series = useMemo(() => {
    if (moves.length === 0) return []
    const points = [{ ply: 0, evalCp: moves[0]?.eval_before_cp ?? 0 }]
    for (const move of moves) {
      points.push({ ply: move.ply, evalCp: move.eval_after_cp ?? move.eval_before_cp ?? 0 })
    }
    return points
  }, [moves])

  const clocks = useMemo(() => {
    let white: number | null = null
    let black: number | null = null
    const user: number[] = []
    const enemy: number[] = []
    for (const move of moves) {
      if (move.color === 'white') white = move.clock_remaining_sec
      else black = move.clock_remaining_sec
      user.push((userColor === 'white' ? white : black) ?? 0)
      enemy.push((userColor === 'white' ? black : white) ?? 0)
    }
    return { user, enemy }
  }, [moves, userColor])

  const x = useCallback(
    (ply: number) => PAD.left + (ply / Math.max(moves.length, 1)) * (WIDTH - PAD.left - PAD.right),
    [moves.length],
  )
  const yEval = (cp: number) => {
    const clamped = Math.max(-600, Math.min(600, cp))
    const normalized = clamped / 600
    return PAD.top + ((1 - (normalized + 1) / 2) * (HEIGHT - PAD.top - PAD.bottom))
  }

  const path = useMemo(() => {
    return series
      .map((point, index) => `${index === 0 ? 'M' : 'L'}${x(point.ply).toFixed(2)},${yEval(point.evalCp).toFixed(2)}`)
      .join(' ')
  }, [series, x])

  const area = `${path} L${x(moves.length)},${HEIGHT - PAD.bottom} L${PAD.left},${HEIGHT - PAD.bottom} Z`
  const maxClock = Math.max(1, ...clocks.user, ...clocks.enemy)
  const yClock = (seconds: number) =>
    PAD.top + (1 - seconds / maxClock) * (HEIGHT - PAD.top - PAD.bottom)

  const clockUserPath = clocks.user
    .map((value, index) => `${index === 0 ? 'M' : 'L'}${x(index + 1).toFixed(2)},${yClock(value).toFixed(2)}`)
    .join(' ')
  const clockEnemyPath = clocks.enemy
    .map((value, index) => `${index === 0 ? 'M' : 'L'}${x(index + 1).toFixed(2)},${yClock(value).toFixed(2)}`)
    .join(' ')

  const selected = moves.find((move) => move.ply === selectedPly)
  const selectedEval = selected?.eval_after_cp ?? moves[0]?.eval_before_cp ?? 0

  function choosePly(clientX: number) {
    const svg = svgRef.current
    if (!svg) return
    const rect = svg.getBoundingClientRect()
    const relative = ((clientX - rect.left) / rect.width) * WIDTH
    const ratio = (relative - PAD.left) / (WIDTH - PAD.left - PAD.right)
    const ply = Math.max(0, Math.min(moves.length, Math.round(ratio * moves.length)))
    onSelect(ply)
  }

  function handleMove(clientX: number) {
    pending.current = clientX
    if (frame.current !== null) return
    frame.current = window.requestAnimationFrame(() => {
      frame.current = null
      if (pending.current !== null) choosePly(pending.current)
    })
  }

  const critical = moves.filter(
    (move) =>
      move.color === userColor &&
      ['mistake', 'blunder', 'missed_win'].includes(move.classification ?? ''),
  )

  return (
    <figure className="eval-chart">
      <div className="eval-chart__head">
        <div>
          <p className="ef-label">Evaluation and clock</p>
          <p className="eval-chart__readout">
            <span className="ef-mono">{selected ? `${selected.move_number}. ${selected.san}` : 'Start'}</span>
            <span className="ef-mono eval-chart__eval">{formatEval(selectedEval, null)}</span>
          </p>
        </div>
        <div className="eval-chart__legend">
          <span><i className="swatch swatch--eval" /> eval</span>
          <span><i className="swatch swatch--clock" /> your clock</span>
          <span><i className="swatch swatch--enemy" /> opponent</span>
        </div>
      </div>
      <svg
        ref={svgRef}
        viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
        className="eval-chart__svg"
        role="img"
        aria-label="Evaluation curve and clock usage by ply. Use left and right arrow keys to scrub."
        tabIndex={0}
        onPointerMove={(event) => handleMove(event.clientX)}
        onPointerDown={(event) => choosePly(event.clientX)}
        onKeyDown={(event) => {
          if (event.key === 'ArrowLeft') {
            event.preventDefault()
            onSelect(Math.max(0, selectedPly - 1))
          } else if (event.key === 'ArrowRight') {
            event.preventDefault()
            onSelect(Math.min(moves.length, selectedPly + 1))
          }
        }}
      >
        <defs>
          <linearGradient id="eval-area" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="#3f5560" stopOpacity="0.22" />
            <stop offset="50%" stopColor="#3f5560" stopOpacity="0.02" />
            <stop offset="100%" stopColor="#a64a24" stopOpacity="0.14" />
          </linearGradient>
        </defs>
        {[2, 0, -2].map((pawns) => {
          const y = yEval(pawns * 100)
          return (
            <g key={pawns}>
              <line x1={PAD.left} x2={WIDTH - PAD.right} y1={y} y2={y} stroke={pawns === 0 ? '#b9ab95' : '#ded3c0'} strokeDasharray={pawns === 0 ? '3 4' : '0'} />
              <text x={PAD.left - 7} y={y + 3} textAnchor="end" fontSize="9" fill="#7c7160" fontFamily="IBM Plex Mono">
                {pawns > 0 ? `+${pawns}` : pawns}
              </text>
            </g>
          )
        })}
        <path d={area} fill="url(#eval-area)" />
        <path d={path} fill="none" stroke="#3f5560" strokeWidth="1.9" />
        <path d={clockUserPath} fill="none" stroke="#3f5560" strokeWidth="1" strokeDasharray="4 4" opacity="0.55" />
        <path d={clockEnemyPath} fill="none" stroke="#a64a24" strokeWidth="1" strokeDasharray="1 4" opacity="0.5" />
        {critical.map((move) => (
          <rect
            key={move.id}
            x={x(move.ply) - 2.5}
            y={yEval(move.eval_after_cp ?? 0) - 2.5}
            width="5"
            height="5"
            fill="#a64a24"
          />
        ))}
        <line
          x1={x(selectedPly)}
          x2={x(selectedPly)}
          y1={PAD.top}
          y2={HEIGHT - PAD.bottom}
          stroke="#242019"
          strokeWidth="1"
          opacity="0.45"
        />
        <circle cx={x(selectedPly)} cy={yEval(selectedEval)} r="4" fill="#242019" stroke="#f3ede0" strokeWidth="1.5" />
        <text x={PAD.left} y={HEIGHT - 6} fontSize="9" fill="#7c7160" fontFamily="IBM Plex Mono">
          start
        </text>
        <text x={WIDTH - PAD.right} y={HEIGHT - 6} textAnchor="end" fontSize="9" fill="#7c7160" fontFamily="IBM Plex Mono">
          move {moves.length}
        </text>
      </svg>
      <figcaption className="sr-only">
        Evaluation starts at {formatEval(moves[0]?.eval_before_cp ?? 0, null)} and ends at{' '}
        {formatEval(moves[moves.length - 1]?.eval_after_cp ?? 0, null)} after {moves.length} plies.
      </figcaption>
    </figure>
  )
}
