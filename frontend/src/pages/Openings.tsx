import { useQuery } from '@tanstack/react-query'
import { ArrowRight } from 'lucide-react'
import { Link } from 'react-router-dom'

import { api, getErrorMessage } from '../api/client'
import type { OpeningReport, OpeningStat } from '../api/types'
import { EmptyState, ErrorBlock, LoadingBlock, PageHeader, Tag } from '../components/ui'
import { formatPercent, shortName } from '../lib/format'

function OpeningTable({ title, rows, color }: { title: string; rows: OpeningStat[]; color: string }) {
  if (rows.length === 0) {
    return (
      <div className="opening-empty">
        <p className="ef-label">{title}</p>
        <p>No analysed {color} games yet.</p>
      </div>
    )
  }
  return (
    <section className="opening-table">
      <div className="section-head">
        <div>
          <p className="ef-label">{title}</p>
          <h2>{rows.length} ECO line{rows.length === 1 ? '' : 's'}</h2>
        </div>
        <span className="ef-mono">{rows.reduce((sum, row) => sum + row.games, 0)} games</span>
      </div>
      <div className="opening-head">
        <span>Opening</span>
        <span>G</span>
        <span>W / D / L</span>
        <span>Win rate</span>
        <span>Acc.</span>
        <span>Leaves book</span>
      </div>
      {rows.map((row) => (
        <Link
          className="opening-row"
          key={`${row.color}-${row.eco}-${row.name}`}
          to={`/games?opening=${encodeURIComponent(row.eco)}`}
        >
          <span className="opening-row__name">
            <span className="ef-mono">{row.eco}</span>
            <strong>{shortName(row.name, 42)}</strong>
            {row.most_punished ? <Tag tone="sienna">most punished</Tag> : null}
            <span className="opening-row__mobile-meta ef-mono">
              {row.games}G · {formatPercent(row.win_rate, 0)} win · {formatPercent(row.accuracy, 0)} acc
            </span>
          </span>
          <span className="ef-mono">{row.games}</span>
          <span className="ef-mono">
            {row.wins} / {row.draws} / {row.losses}
          </span>
          <span className="opening-row__rate">
            <span className="mini-bar" aria-hidden="true">
              <i style={{ width: `${row.win_rate}%` }} />
            </span>
            <span className="ef-mono">{formatPercent(row.win_rate, 0)}</span>
          </span>
          <span className="ef-mono">{formatPercent(row.accuracy, 0)}</span>
          <span className="ef-mono">move {row.out_of_book_move.toFixed(1)}</span>
        </Link>
      ))}
    </section>
  )
}

export default function Openings() {
  const query = useQuery({
    queryKey: ['reports', 'openings'],
    queryFn: () => api<OpeningReport>('/api/reports/openings'),
  })

  if (query.isLoading) return <LoadingBlock label="Building the repertoire matrix…" />
  if (query.isError) return <ErrorBlock message={getErrorMessage(query.error)} retry={() => void query.refetch()} />
  const data = query.data
  if (!data) return null

  const total = data.white.length + data.black.length
  const priorities = [...data.white, ...data.black]
    .filter((row) => row.games >= 2)
    .sort((a, b) => a.win_rate - b.win_rate)
    .slice(0, 3)

  return (
    <>
      <PageHeader
        eyebrow="Report 03 · Opening repertoire"
        title="Where the book ends and the trouble begins."
        description="Win rate, accuracy and the average move where you leave theory. Split by colour because they are different jobs."
        actions={<Link className="ef-btn ef-btn--ghost" to="/app">Back to Today</Link>}
      />

      {total === 0 ? (
        <EmptyState title="No opening data yet">
          Sync and analyse a few games. EloForge matches every move against the Lichess ECO book (A00–E99) and records the last book ply.
        </EmptyState>
      ) : (
        <>
          {priorities.length > 0 ? (
            <section className="opening-priorities" aria-labelledby="opening-priorities-title">
              <div className="opening-priorities__head">
                <p className="ef-label">Start here</p>
                <h2 id="opening-priorities-title">Three lines deserve a review first.</h2>
              </div>
              <ol>
                {priorities.map((line) => (
                  <li key={`${line.color}-${line.eco}-${line.name}`}>
                    <Link to={`/games?opening=${encodeURIComponent(line.eco)}`}>
                      <span className="ef-mono">{line.eco}</span>
                      <strong>{shortName(line.name, 36)}</strong>
                      <span className="ef-mono">{formatPercent(line.win_rate, 0)} win · {line.games} games</span>
                      <span className="opening-priorities__cta">Review games <ArrowRight size={13} /></span>
                    </Link>
                  </li>
                ))}
              </ol>
            </section>
          ) : null}

          <section className="opening-advice">
            <p className="ef-label">Elo-appropriate repertoire advice</p>
            <ol>
              {data.recommendations.map((recommendation, index) => (
                <li key={recommendation}>
                  <span className="ef-mono">{String(index + 1).padStart(2, '0')}</span>
                  <p>{recommendation}</p>
                </li>
              ))}
            </ol>
          </section>

          <div className="opening-grid">
            <OpeningTable title="White repertoire" rows={data.white} color="White" />
            <OpeningTable title="Black repertoire" rows={data.black} color="Black" />
          </div>

          <section className="opening-note">
            <p className="ef-label">How to read “book ends”</p>
            <p>
              “Leaves book” is the average move where you leave the ECO database. A low number is not automatically
              bad; it means the next moves are your own plan, not memory. “Most punished” marks lines with at least
              three games where losses equal or exceed wins. Click any line to review those games.
            </p>
          </section>
        </>
      )}
    </>
  )
}
