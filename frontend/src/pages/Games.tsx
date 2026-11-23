import { useQuery } from '@tanstack/react-query'
import { ChevronLeft, ChevronRight, Filter, X } from 'lucide-react'
import { useMemo } from 'react'
import { Link, useSearchParams } from 'react-router-dom'

import { api, getErrorMessage } from '../api/client'
import type { GamePage, OpeningReport } from '../api/types'
import { EmptyState, ErrorBlock, LoadingBlock, PageHeader, ResultMark, Tag } from '../components/ui'
import { formatDate, formatPercent, resultLabel, shortName } from '../lib/format'

const PAGE_SIZE = 20

export default function Games() {
  const [params, setParams] = useSearchParams()
  const result = params.get('result') ?? ''
  const status = params.get('status') ?? ''
  const opening = params.get('opening') ?? ''
  const timeClass = params.get('time') ?? ''
  const page = Math.max(1, Number(params.get('page') ?? '1') || 1)

  const gameQuery = useMemo(() => {
    const query = new URLSearchParams()
    query.set('limit', String(PAGE_SIZE))
    query.set('offset', String((page - 1) * PAGE_SIZE))
    if (result) query.set('result', result)
    if (status) query.set('status', status)
    if (opening) query.set('opening', opening)
    if (timeClass) query.set('time_class', timeClass)
    return query.toString()
  }, [opening, page, result, status, timeClass])

  const games = useQuery({
    queryKey: ['games', 'index', gameQuery],
    queryFn: () => api<GamePage>(`/api/games?${gameQuery}`),
  })
  const openingReport = useQuery({
    queryKey: ['reports', 'openings'],
    queryFn: () => api<OpeningReport>('/api/reports/openings'),
    staleTime: 60_000,
  })

  const openingOptions = useMemo(() => {
    const seen = new Map<string, string>()
    for (const row of [...(openingReport.data?.white ?? []), ...(openingReport.data?.black ?? [])]) {
      if (!seen.has(row.eco)) seen.set(row.eco, `${row.eco} · ${row.name}`)
    }
    return [...seen.entries()].sort(([a], [b]) => a.localeCompare(b))
  }, [openingReport.data])

  function update(key: string, value: string) {
    const next = new URLSearchParams(params)
    if (value) next.set(key, value)
    else next.delete(key)
    next.delete('page')
    setParams(next, { replace: true })
  }

  function goPage(nextPage: number) {
    const next = new URLSearchParams(params)
    if (nextPage <= 1) next.delete('page')
    else next.set('page', String(nextPage))
    setParams(next, { replace: true })
  }

  const activeFilters = [result, status, opening, timeClass].filter(Boolean).length
  const total = games.data?.total ?? 0
  const pages = Math.max(1, Math.ceil(total / PAGE_SIZE))

  return (
    <>
      <PageHeader
        eyebrow="Review · Every finished game"
        title="Find the game that teaches you something."
        description="Filter by result, opening or time control, then open a guided review at the first critical moment."
        actions={
          activeFilters > 0 ? (
            <button className="ef-btn ef-btn--ghost" type="button" onClick={() => setParams(new URLSearchParams(), { replace: true })}>
              <X size={14} /> Clear filters
            </button>
          ) : null
        }
      />

      <div className="games-filters" aria-label="Game filters">
        <span className="ef-label"><Filter size={13} /> Filters</span>
        <label>
          <span className="sr-only">Result</span>
          <select className="ef-input" value={result} onChange={(event) => update('result', event.target.value)}>
            <option value="">All results</option>
            <option value="win">Wins</option>
            <option value="loss">Losses</option>
            <option value="draw">Draws</option>
          </select>
        </label>
        <label>
          <span className="sr-only">Time class</span>
          <select className="ef-input" value={timeClass} onChange={(event) => update('time', event.target.value)}>
            <option value="">All time controls</option>
            <option value="rapid">Rapid</option>
            <option value="blitz">Blitz</option>
            <option value="bullet">Bullet</option>
            <option value="daily">Daily</option>
          </select>
        </label>
        <label>
          <span className="sr-only">Analysis status</span>
          <select className="ef-input" value={status} onChange={(event) => update('status', event.target.value)}>
            <option value="">All analysis states</option>
            <option value="done">Analysed</option>
            <option value="pending">Pending</option>
            <option value="queued">Queued</option>
            <option value="analyzing">Analysing</option>
            <option value="failed">Failed</option>
          </select>
        </label>
        <label className="games-filters__opening">
          <span className="sr-only">Opening</span>
          <select className="ef-input" value={opening} onChange={(event) => update('opening', event.target.value)}>
            <option value="">All openings</option>
            {openingOptions.map(([eco, label]) => (
              <option key={eco} value={eco}>{label}</option>
            ))}
          </select>
        </label>
        <span className="ef-mono games-filters__count">{total} game{total === 1 ? '' : 's'}</span>
      </div>

      {games.isLoading ? <LoadingBlock label="Loading the game archive…" /> : null}
      {games.isError ? <ErrorBlock message={getErrorMessage(games.error)} retry={() => void games.refetch()} /> : null}

      {games.data && games.data.items.length === 0 ? (
        <EmptyState title="No games match these filters">
          Clear the filters, or sync a fresh archive from Chess.com.
        </EmptyState>
      ) : null}

      {games.data && games.data.items.length > 0 ? (
        <div className="ledger games-ledger" role="table" aria-label="Games">
          <div className="ledger__head" role="row">
            <span role="columnheader">Result</span>
            <span role="columnheader">Opponent</span>
            <span role="columnheader">Opening</span>
            <span role="columnheader">Accuracy</span>
            <span role="columnheader">Status</span>
            <span role="columnheader">Date</span>
          </div>
          {games.data.items.map((game) => {
            const opponent = game.user_color === 'white' ? game.black_username : game.white_username
            return (
              <Link className="ledger__row" role="row" to={`/games/${game.id}`} key={game.id}>
                <span role="cell" className="ledger__result">
                  <ResultMark result={game.result} />
                  <span>{resultLabel(game.result)}</span>
                </span>
                <span role="cell" className="ledger__opponent">
                  {shortName(opponent, 22)}
                  <small className="ef-mono">
                    {game.user_color === 'white' ? game.black_rating : game.white_rating} · {formatDate(game.start_time)}
                  </small>
                </span>
                <span role="cell" className="ledger__opening">
                  {game.opening_name ?? game.opening_eco ?? 'Unknown'}
                  <small className="ef-mono">{game.opening_eco ?? '—'}</small>
                </span>
                <span role="cell" className="ef-mono ledger__accuracy">{formatPercent(game.accuracy_user, 0)}</span>
                <span role="cell"><Tag tone={game.analysis_status === 'done' ? 'good' : 'neutral'}>{game.analysis_status}</Tag></span>
                <span role="cell" className="ledger__date ef-mono">{formatDate(game.start_time)}</span>
              </Link>
            )
          })}
        </div>
      ) : null}

      {pages > 1 ? (
        <nav className="games-pagination" aria-label="Pages">
          <button className="ef-btn ef-btn--ghost" type="button" disabled={page <= 1} onClick={() => goPage(page - 1)}>
            <ChevronLeft size={14} /> Previous
          </button>
          <span className="ef-mono">Page {page} of {pages}</span>
          <button className="ef-btn ef-btn--ghost" type="button" disabled={page >= pages} onClick={() => goPage(page + 1)}>
            Next <ChevronRight size={14} />
          </button>
        </nav>
      ) : null}
    </>
  )
}
