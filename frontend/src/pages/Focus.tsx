import { useQuery } from '@tanstack/react-query'
import { ArrowRight } from 'lucide-react'
import { Link } from 'react-router-dom'

import { api, getErrorMessage } from '../api/client'
import type { FocusReport } from '../api/types'
import { EmptyState, ErrorBlock, LoadingBlock, PageHeader, StatBlock } from '../components/ui'
import { formatPercent } from '../lib/format'
import { getDisplayTimeZone, timeZoneLabel } from '../lib/timezone'

function ImpulseScatter({ report }: { report: FocusReport }) {
  const width = 680
  const height = 280
  const pad = { top: 18, right: 24, bottom: 34, left: 42 }
  const maxSeconds = Math.max(15, ...report.time_buckets.map((bucket) => bucket.midpoint_seconds))
  const x = (seconds: number) => pad.left + (seconds / maxSeconds) * (width - pad.left - pad.right)
  const y = (rate: number) => pad.top + (1 - rate / 100) * (height - pad.top - pad.bottom)
  const maxMoves = Math.max(1, ...report.time_buckets.map((bucket) => bucket.moves))

  return (
    <figure className="scatter">
      <div className="scatter__head">
        <div>
          <p className="ef-label">Impulsivity map</p>
          <p className="scatter__title">Move time vs blunder rate</p>
        </div>
        <p className="scatter__note">Bubble size = moves in that band</p>
      </div>
      <svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label="Scatter plot of move time against blunder rate">
        {[0, 25, 50, 75, 100].map((rate) => (
          <g key={rate}>
            <line x1={pad.left} x2={width - pad.right} y1={y(rate)} y2={y(rate)} stroke="#ded3c0" />
            <text x={pad.left - 8} y={y(rate) + 3} textAnchor="end" fontSize="9" fill="#7c7160" fontFamily="IBM Plex Mono">
              {rate}%
            </text>
          </g>
        ))}
        {report.time_buckets.map((bucket) => (
          <g key={bucket.label}>
            <circle
              cx={x(bucket.midpoint_seconds)}
              cy={y(bucket.blunder_rate)}
              r={4 + (bucket.moves / maxMoves) * 10}
              fill={bucket.blunder_rate > 40 ? '#a64a24' : bucket.blunder_rate > 20 ? '#b98a3e' : '#3f5560'}
              fillOpacity="0.82"
              stroke="#f3ede0"
              strokeWidth="1.2"
            >
              <title>{`${bucket.label}: ${bucket.moves} moves, ${bucket.blunder_rate.toFixed(0)}% blunders, ${bucket.accuracy.toFixed(0)}% accuracy`}</title>
            </circle>
            <text
              x={x(bucket.midpoint_seconds)}
              y={height - pad.bottom + 15}
              textAnchor="middle"
              fontSize="9"
              fill="#7c7160"
              fontFamily="IBM Plex Mono"
            >
              {bucket.label}
            </text>
          </g>
        ))}
        <line x1={pad.left} x2={width - pad.right} y1={height - pad.bottom} y2={height - pad.bottom} stroke="#b9ab95" />
      </svg>
      <figcaption className="sr-only">
        Blunder rate by time spent: {report.time_buckets.map((bucket) => `${bucket.label} ${bucket.blunder_rate.toFixed(0)} percent`).join(', ')}.
      </figcaption>
      <details className="chart-data">
        <summary>Show the time-band data as a table</summary>
        <table>
          <thead><tr><th>Band</th><th>Moves</th><th>Blunder rate</th><th>Accuracy</th></tr></thead>
          <tbody>
            {report.time_buckets.map((bucket) => (
              <tr key={bucket.label}>
                <td>{bucket.label}</td>
                <td>{bucket.moves}</td>
                <td>{formatPercent(bucket.blunder_rate, 0)}</td>
                <td>{formatPercent(bucket.accuracy, 0)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </details>
    </figure>
  )
}

function TiltCurve({ report }: { report: FocusReport }) {
  const width = 400
  const height = 160
  const pad = { top: 12, right: 12, bottom: 26, left: 30 }
  const x = (index: number) => pad.left + ((index - 1) / 2) * (width - pad.left - pad.right)
  const y = (accuracy: number) => pad.top + (1 - accuracy / 100) * (height - pad.top - pad.bottom)
  const points = report.session_curve
    .filter((slot) => slot.games > 0)
    .map((slot) => `${x(slot.index)},${y(slot.accuracy)}`)
    .join(' ')

  return (
    <figure className="tilt-curve">
      <p className="ef-label">Session tilt</p>
      <svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label="Accuracy by game index within a session">
        <line x1={pad.left} x2={width - pad.right} y1={y(50)} y2={y(50)} stroke="#ded3c0" strokeDasharray="3 4" />
        <polyline points={points} fill="none" stroke="#a64a24" strokeWidth="2" />
        {report.session_curve.map((slot) => (
          <g key={slot.index}>
            {slot.games > 0 ? <circle cx={x(slot.index)} cy={y(slot.accuracy)} r="3.5" fill="#a64a24" /> : null}
            <text x={x(slot.index)} y={height - 8} textAnchor="middle" fontSize="9" fill="#7c7160" fontFamily="IBM Plex Mono">
              {slot.index === 3 ? '3+' : slot.index}
            </text>
          </g>
        ))}
      </svg>
      <figcaption>
        {report.session_curve.map((slot) => (
          <span key={slot.index} className="ef-mono">
            G{slot.index === 3 ? '3+' : slot.index}:{' '}
            {slot.games > 0
              ? `${formatPercent(slot.accuracy, 0)} / ${formatPercent(slot.win_rate, 0)} win`
              : 'no games'}
          </span>
        ))}
      </figcaption>
      <details className="chart-data">
        <summary>Show the session data as a table</summary>
        <table>
          <thead><tr><th>Game in session</th><th>Games</th><th>Accuracy</th><th>Win rate</th></tr></thead>
          <tbody>
            {report.session_curve.map((slot) => (
              <tr key={slot.index}>
                <td>{slot.index === 3 ? '3+' : slot.index}</td>
                <td>{slot.games}</td>
                <td>{formatPercent(slot.accuracy, 0)}</td>
                <td>{formatPercent(slot.win_rate, 0)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </details>
    </figure>
  )
}

function heatColor(accuracy: number, games: number): string {
  if (games === 0) return '#e9e0ce'
  const t = Math.max(0, Math.min(1, accuracy / 100))
  const light = 78 - t * 34
  return `hsl(22 ${18 + t * 34}% ${light}%)`
}

export default function Focus() {
  const timeZone = getDisplayTimeZone()
  const query = useQuery({
    queryKey: ['focus', timeZone],
    queryFn: () => api<FocusReport>(`/api/focus?tz=${encodeURIComponent(timeZone)}`),
  })

  if (query.isLoading) return <LoadingBlock label="Reading the clock data…" />
  if (query.isError) return <ErrorBlock message={getErrorMessage(query.error)} retry={() => void query.refetch()} />
  const report = query.data
  if (!report) return null

  if (report.total_user_moves === 0) {
    return (
      <>
        <PageHeader eyebrow="Report 06 · Focus, speed & tilt" title="The clock is part of the position." />
        <EmptyState title="No analysed moves yet" action={<Link className="ef-btn ef-btn--primary" to="/app">Sync and analyse</Link>}>
          Focus metrics come from real clock comments in your PGNs. Import a few Rapid games first.
        </EmptyState>
      </>
    )
  }

  const worstBand = [...report.time_buckets].sort((a, b) => b.blunder_rate - a.blunder_rate)[0]
  const bestBand = [...report.time_buckets].filter((bucket) => bucket.moves > 2).sort((a, b) => a.blunder_rate - b.blunder_rate)[0]

  return (
    <>
      <PageHeader
        eyebrow="Report 06 · Focus, speed & tilt"
        title="The clock is part of the position."
        description="Impulsive blunders, post-blunder cascades, session tilt and the hours when your calculation actually works."
        actions={<Link className="ef-btn ef-btn--ghost" to="/app">Back to Today</Link>}
      />

      <section className="focus-verdict" aria-label="Focus verdict">
        <div>
          <p className="ef-label">The one thing to change</p>
          <h2>
            {worstBand && worstBand.moves > 0
              ? `Your ${worstBand.label} moves are the weak point.`
              : 'Keep gathering clock data.'}
          </h2>
          <p>
            {worstBand
              ? `${formatPercent(worstBand.blunder_rate, 0)} of moves in this band are tagged mistakes or worse. ${bestBand ? `Your cleanest band is ${bestBand.label}.` : ''}`
              : 'Import a few Rapid games so the clock can tell an honest story.'}
          </p>
        </div>
        <div className="focus-verdict__actions">
          <Link className="ef-btn ef-btn--primary" to="/puzzles?tag=TACTICAL_BLINDNESS">
            Practise the pattern <ArrowRight size={14} />
          </Link>
          <Link className="ef-btn ef-btn--ghost" to="/games">
            Review the games
          </Link>
        </div>
      </section>

      <section className="focus-stats">
        <StatBlock
          label="Moves played too fast"
          value={formatPercent(report.impulsivity_index * 100, 1)}
          note={`${report.impulsive_blunders} of ${report.total_user_moves} moves were quick mistakes`}
          tone="sienna"
        />
        <StatBlock
          label="Post-blunder cascade"
          value={formatPercent(report.post_blunder_cascade_rate * 100, 0)}
          note={`${report.cascade_instances} blunders followed by another within 3 moves`}
          tone="brass"
        />
        <StatBlock
          label="Session tilt drop"
          value={(report.session_curve[2]?.games ?? 0) >= 2 ? formatPercent(report.session_tilt_drop, 1) : '—'}
          note={
            (report.session_curve[2]?.games ?? 0) >= 2
              ? `Game 1 ${formatPercent(report.game1_accuracy, 0)} → Game 3+ ${formatPercent(report.game3plus_accuracy, 0)}`
              : 'Need third games in a sitting'
          }
          tone="slate"
        />
        <StatBlock
          label="Blunders tagged"
          value={report.blunder_count}
          note={`of ${report.total_user_moves} analysed moves`}
        />
      </section>

      <div className="focus-grid">
        <section className="focus-panel">
          <ImpulseScatter report={report} />
          <p className="focus-insight">
            {worstBand
              ? `Your worst band is ${worstBand.label}: ${formatPercent(worstBand.blunder_rate, 0)} of those moves are tagged mistakes or worse.`
              : 'No move-time data yet.'}{' '}
            {bestBand
              ? `The cleanest band is ${bestBand.label} at ${formatPercent(bestBand.blunder_rate, 0)}.`
              : ''}
          </p>
        </section>

        <section className="focus-panel">
          <TiltCurve report={report} />
          <p className="focus-insight">
            {(report.session_curve[2]?.games ?? 0) < 2
              ? 'Not enough third games in a session yet. Play a short block of three to make this curve honest.'
              : report.session_tilt_drop > 8
                ? 'Accuracy drops noticeably by the third game. Apply the two-loss stop rule before the next session.'
                : 'Accuracy holds across the session. Your study routine is not being undone by fatigue.'}
          </p>
        </section>
      </div>

      <section className="heatmap-panel">
        <div className="section-head">
          <div>
            <p className="ef-label">Chronotype</p>
            <h2>Accuracy by hour of day</h2>
          </div>
          <span className="ef-mono">{timeZoneLabel(report.timezone)} · {report.hour_heatmap.reduce((sum, hour) => sum + hour.games, 0)} games</span>
        </div>
        <div className="heatmap" role="img" aria-label="Accuracy heatmap by hour of day">
          {report.hour_heatmap.map((hour) => (
            <div
              key={hour.hour}
              className="heatmap__cell"
              style={{ background: heatColor(hour.accuracy, hour.games) }}
              title={`${String(hour.hour).padStart(2, '0')}:00 — ${hour.games} game(s), ${formatPercent(hour.accuracy, 0)} accuracy`}
            >
              {hour.games > 0 ? (
                <span className="ef-mono heatmap__value">{Math.round(hour.accuracy)}</span>
              ) : null}
              {hour.hour % 3 === 0 ? <small className="ef-mono">{String(hour.hour).padStart(2, '0')}</small> : null}
            </div>
          ))}
        </div>
        <div className="heatmap-key">
          <span className="ef-label">low</span>
          <span className="heatmap-key__swatch" style={{ background: heatColor(35, 1) }} />
          <span className="heatmap-key__swatch" style={{ background: heatColor(55, 1) }} />
          <span className="heatmap-key__swatch" style={{ background: heatColor(75, 1) }} />
          <span className="heatmap-key__swatch" style={{ background: heatColor(95, 1) }} />
          <span className="ef-label">high</span>
        </div>
        <details className="chart-data">
          <summary>Show the hourly data as a table</summary>
          <table>
            <thead><tr><th>Hour</th><th>Games</th><th>Accuracy</th><th>Win rate</th></tr></thead>
            <tbody>
              {report.hour_heatmap.filter((hour) => hour.games > 0).map((hour) => (
                <tr key={hour.hour}>
                  <td>{String(hour.hour).padStart(2, '0')}:00</td>
                  <td>{hour.games}</td>
                  <td>{formatPercent(hour.accuracy, 0)}</td>
                  <td>{formatPercent(hour.win_rate, 0)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </details>
      </section>

      <section className="focus-actions">
        <div>
          <p className="ef-label">What to do next</p>
          <h2>One drill, one review, one rule.</h2>
        </div>
        <ol>
          <li><span className="ef-mono">01</span><span>Open the weakest time band as a puzzle filter and clear five drills.</span><Link to="/puzzles?tag=TACTICAL_BLINDNESS">Practise <ArrowRight size={13} /></Link></li>
          <li><span className="ef-mono">02</span><span>Review one loss where you had more than half your clock left.</span><Link to="/games?result=loss">Review <ArrowRight size={13} /></Link></li>
          <li><span className="ef-mono">03</span><span>Use the two-loss stop rule before the next session.</span><Link to="/roadmap">Set a goal <ArrowRight size={13} /></Link></li>
        </ol>
      </section>

      <section className="depth-panel">
        <div>
          <p className="ef-label">Move-depth fatigue</p>
          <h2>Accuracy as the game gets longer</h2>
        </div>
        <div className="depth-bars">
          {report.depth_fatigue.map((bucket) => (
            <div key={bucket.label} className="depth-row">
              <span className="ef-mono">{bucket.label}</span>
              <div className="phase-bar"><span style={{ width: `${bucket.accuracy}%` }} /></div>
              <span className="ef-mono">{formatPercent(bucket.accuracy, 0)}</span>
              <span className="ef-mono depth-row__moves">{bucket.moves} moves</span>
            </div>
          ))}
        </div>
      </section>
    </>
  )
}
