import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { ArrowRight, BookOpen, Sparkles } from 'lucide-react'
import { useState, type ReactNode } from 'react'
import { Link } from 'react-router-dom'

import { api, getErrorMessage } from '../api/client'
import type {
  GamePage,
  PuzzleStats,
  RoadmapView,
  SyncStatus,
  WeaknessReport,
  WeeklyReport,
} from '../api/types'
import SyncButton from '../components/SyncButton'
import { EmptyState, ErrorBlock, LoadingBlock, PageHeader, ResultMark, StatBlock, Tag } from '../components/ui'
import { useAuth } from '../hooks/useAuth'
import {
  formatDate,
  formatPercent,
  relativeTime,
  resultLabel,
  shortName,
} from '../lib/format'

function inline(text: string): ReactNode[] {
  const parts = text.split(/(\*\*[^*]+\*\*)/g)
  return parts.filter(Boolean).map((part, index) =>
    part.startsWith('**') && part.endsWith('**') ? (
      <strong key={index}>{part.slice(2, -2)}</strong>
    ) : (
      <span key={index}>{part}</span>
    ),
  )
}

function Prose({ text }: { text: string }) {
  return (
    <div className="prose">
      {text.split('\n').map((line, index) => {
        const trimmed = line.trim()
        if (!trimmed) return <br key={index} />
        if (/^\*\*[^*]+\*\*$/.test(trimmed)) {
          return <h3 key={index}>{trimmed.slice(2, -2)}</h3>
        }
        if (/^\d+\./.test(trimmed) || trimmed.startsWith('- ')) {
          return <p key={index} className="prose__item">{inline(trimmed.replace(/^-\s/, ''))}</p>
        }
        return <p key={index}>{inline(trimmed)}</p>
      })}
    </div>
  )
}

export default function Dashboard() {
  const { user } = useAuth()
  const queryClient = useQueryClient()
  const [now] = useState(() => new Date())

  const sync = useQuery({
    queryKey: ['sync'],
    queryFn: () => api<SyncStatus>('/api/sync/status'),
  })
  const games = useQuery({
    queryKey: ['games', 'recent'],
    queryFn: () => api<GamePage>('/api/games?limit=6'),
  })
  const puzzles = useQuery({
    queryKey: ['puzzles', 'stats'],
    queryFn: () => api<PuzzleStats>('/api/puzzles/stats'),
  })
  const weaknesses = useQuery({
    queryKey: ['reports', 'weaknesses'],
    queryFn: () => api<WeaknessReport>('/api/reports/weaknesses'),
  })
  const roadmap = useQuery({
    queryKey: ['roadmap'],
    queryFn: () => api<RoadmapView>('/api/roadmap'),
  })
  const weekly = useQuery({
    queryKey: ['reports', 'weekly', 'latest'],
    queryFn: () => api<WeeklyReport>('/api/reports/weekly/latest'),
    retry: false,
  })

  const generate = useMutation({
    mutationFn: () => api<WeeklyReport>('/api/reports/weekly/generate', { method: 'POST' }),
    onSuccess: (data) => {
      queryClient.setQueryData(['reports', 'weekly', 'latest'], data)
      void queryClient.invalidateQueries({ queryKey: ['reports'] })
    },
  })

  const firstName = user?.display_name?.split(' ')[0] ?? user?.chesscom_username ?? 'player'
  const hour = now.getHours()
  const greeting = hour < 12 ? 'Good morning' : hour < 18 ? 'Good afternoon' : 'Good evening'
  const stage = roadmap.data?.stages.find((item) => item.current)
  const topLeaks = weaknesses.data?.top_three ?? []
  const latestGames = games.data?.items ?? []
  const due = puzzles.data?.due ?? 0

  return (
    <>
      <PageHeader
        eyebrow={new Intl.DateTimeFormat('en-GB', {
          weekday: 'long',
          day: '2-digit',
          month: 'long',
        }).format(now)}
        title={`${greeting}, ${firstName}.`}
        description="One board, one habit, one honest measurement at a time."
        actions={<SyncButton compact />}
      />

      {games.isLoading ? <LoadingBlock /> : null}
      {games.isError ? <ErrorBlock message="Could not load your games." retry={() => void games.refetch()} /> : null}

      {games.data && games.data.total === 0 ? (
        <EmptyState
          title="Your study room is empty."
          action={<SyncButton />}
        >
          Connect your Chess.com account, then sync. EloForge will import your finished games serially and build the first evidence set.
        </EmptyState>
      ) : (
        <>
          <section className="dash-hero">
            <div className="dash-hero__rating">
              <p className="ef-label">Current Rapid</p>
              <p className="dash-hero__number">{user?.current_elo ?? '—'}</p>
              <p className="dash-hero__sub">
                {stage ? `Stage ${stage.stage}: ${stage.title}` : 'Roadmap not seeded yet'}
              </p>
            </div>
            <div className="dash-hero__progress">
              <div className="dash-hero__progress-head">
                <span className="ef-label">Next milestone</span>
                <span className="ef-mono">{roadmap.data?.next_target ?? '—'}</span>
              </div>
              <div className="progress-rule" aria-hidden="true">
                <span style={{ width: `${roadmap.data?.progress_pct ?? 0}%` }} />
              </div>
              <div className="dash-hero__progress-foot">
                <span>{formatPercent(roadmap.data?.progress_pct ?? 0, 0)} of the gap closed</span>
                <Link to="/roadmap">Open the ladder <ArrowRight size={13} /></Link>
              </div>
              <div className="dash-hero__mini">
                <span><strong className="ef-mono">{games.data?.total ?? 0}</strong> games stored</span>
                <span><strong className="ef-mono">{sync.data?.analyzed_games ?? 0}</strong> analysed</span>
                <span><strong className="ef-mono">{sync.data?.pending_analysis ?? 0}</strong> in queue</span>
              </div>
            </div>
            <div className="dash-hero__queue">
              <StatBlock
                label="Due blunder drills"
                value={due}
                note={due > 0 ? 'Queue built from your own mistakes' : 'Queue is clear'}
                tone="brass"
              />
              <Link className="ef-btn ef-btn--primary" to="/puzzles">
                {due > 0 ? "Start today's set" : 'Open puzzle trainer'}
                <ArrowRight size={15} />
              </Link>
              <p className="dash-hero__sync">
                Last sync {relativeTime(sync.data?.last_sync_at)} ·{' '}
                {sync.data?.running ? 'running now' : 'idle'}
              </p>
            </div>
          </section>

          <div className="dash-columns">
            <section className="dash-section">
              <div className="section-head">
                <div>
                  <p className="ef-label">Rating leaks</p>
                  <h2>What is actually costing you games</h2>
                </div>
                <Link className="section-head__link" to="/focus">
                  Full diagnosis <ArrowRight size={13} />
                </Link>
              </div>
              {topLeaks.length === 0 ? (
                <EmptyState title="No tagged mistakes yet">
                  Analyse a game first. Tags come from deterministic rules, not a model's opinion.
                </EmptyState>
              ) : (
                <ol className="leak-list">
                  {topLeaks.map((leak, index) => (
                    <li key={leak.tag}>
                      <span className="leak-list__rank ef-mono">{String(index + 1).padStart(2, '0')}</span>
                      <div className="leak-list__body">
                        <div className="leak-list__head">
                          <h3>{leak.label}</h3>
                          <span className={`trend trend--${leak.trend}`}>{leak.trend}</span>
                        </div>
                        <p>{leak.advice}</p>
                        <div className="leak-list__meta ef-mono">
                          <span>{leak.count} tagged</span>
                          <span>avg {formatPercent(leak.avg_epl * 100, 1)} win lost</span>
                          <span>≈ {Math.round(leak.estimated_elo_cost)} Elo</span>
                        </div>
                      </div>
                    </li>
                  ))}
                </ol>
              )}
            </section>

            <aside className="dash-aside">
              <div className="briefing">
                <div className="briefing__head">
                  <p className="ef-label">Weekly coach briefing</p>
                  {weekly.data ? (
                    <span className="ef-mono briefing__status">{weekly.data.ai_status}</span>
                  ) : null}
                </div>
                {weekly.isLoading ? <LoadingBlock label="Reading the week…" /> : null}
                {weekly.data ? (
                  <>
                    <Prose text={weekly.data.summary} />
                    <div className="briefing__rule">
                      <span className="ef-label">Next-week rule</span>
                      <p>{weekly.data.next_week_rule}</p>
                    </div>
                  </>
                ) : (
                  <>
                    <p className="briefing__empty">
                      No briefing for this week yet. Generate one from the verified facts; without an OpenCode key it stays deterministic.
                    </p>
                    <button
                      className="ef-btn ef-btn--ghost"
                      type="button"
                      onClick={() => generate.mutate()}
                      disabled={generate.isPending}
                    >
                      <Sparkles size={15} />
                      {generate.isPending ? 'Writing…' : 'Generate briefing'}
                    </button>
                    {generate.error ? <p className="form-error">{getErrorMessage(generate.error)}</p> : null}
                  </>
                )}
              </div>

              <div className="study-note">
                <BookOpen size={18} strokeWidth={1.6} />
                <div>
                  <p className="ef-label">Stage {stage?.stage ?? 1} focus</p>
                  <p>{stage?.description ?? 'Stop giving away free pieces.'}</p>
                  <ul>
                    {(stage?.criteria ?? []).slice(0, 2).map((criterion) => (
                      <li key={criterion}>{criterion}</li>
                    ))}
                  </ul>
                </div>
              </div>
            </aside>
          </div>

          <section className="dash-section">
            <div className="section-head">
              <div>
                <p className="ef-label">Recent games</p>
                <h2>The evidence ledger</h2>
              </div>
              <span className="section-head__link ef-mono">{games.data?.total ?? 0} total</span>
            </div>
            <div className="ledger" role="table" aria-label="Recent games">
              <div className="ledger__head" role="row">
                <span role="columnheader">Result</span>
                <span role="columnheader">Opponent</span>
                <span role="columnheader">Opening</span>
                <span role="columnheader">Accuracy</span>
                <span role="columnheader">Status</span>
                <span role="columnheader" aria-label="Open" />
              </div>
              {latestGames.map((game) => {
                const opponent =
                  game.user_color === 'white' ? game.black_username : game.white_username
                return (
                  <Link className="ledger__row" role="row" to={`/games/${game.id}`} key={game.id}>
                    <span role="cell" className="ledger__result">
                      <ResultMark result={game.result} />
                      <span>{resultLabel(game.result)}</span>
                    </span>
                    <span role="cell" className="ledger__opponent">
                      {shortName(opponent, 22)}
                      <small className="ef-mono">
                        {game.user_color === 'white' ? game.black_rating : game.white_rating}
                      </small>
                    </span>
                    <span role="cell" className="ledger__opening">
                      {game.opening_name ?? game.opening_eco ?? 'Unknown'}
                      <small className="ef-mono">{game.opening_eco ?? '—'}</small>
                    </span>
                    <span role="cell" className="ef-mono ledger__accuracy">
                      {formatPercent(game.accuracy_user, 0)}
                    </span>
                    <span role="cell">
                      <Tag tone={game.analysis_status === 'done' ? 'good' : 'neutral'}>
                        {game.analysis_status}
                      </Tag>
                    </span>
                    <span role="cell" className="ledger__date ef-mono">
                      {formatDate(game.start_time)}
                    </span>
                  </Link>
                )
              })}
            </div>
          </section>
        </>
      )}
    </>
  )
}
