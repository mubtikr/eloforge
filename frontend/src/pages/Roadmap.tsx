import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Check, Plus, Sparkles } from 'lucide-react'
import { useState, type FormEvent } from 'react'

import { api, getErrorMessage } from '../api/client'
import type { CurriculumReport, GoalDto, RoadmapStageView, RoadmapView } from '../api/types'
import { EmptyState, ErrorBlock, LoadingBlock, PageHeader, StatBlock } from '../components/ui'
import { formatPercent } from '../lib/format'

interface StageProgress {
  criteria_met?: boolean
  metrics?: Record<string, number | null>
}

export default function Roadmap() {
  const queryClient = useQueryClient()
  const [title, setTitle] = useState('')
  const [targetElo, setTargetElo] = useState('')

  const roadmap = useQuery({
    queryKey: ['roadmap'],
    queryFn: () => api<RoadmapView>('/api/roadmap'),
  })
  const goals = useQuery({
    queryKey: ['goals'],
    queryFn: () => api<GoalDto[]>('/api/goals'),
  })
  const curriculum = useQuery({
    queryKey: ['reports', 'curriculum'],
    queryFn: () => api<CurriculumReport>('/api/reports/curriculum/generate', { method: 'POST' }),
    enabled: false,
    retry: false,
  })

  const createGoal = useMutation({
    mutationFn: () =>
      api<GoalDto>('/api/goals', {
        method: 'POST',
        body: JSON.stringify({
          title,
          description: null,
          target_elo: targetElo ? Number(targetElo) : null,
          target_date: null,
        }),
      }),
    onSuccess: () => {
      setTitle('')
      setTargetElo('')
      void queryClient.invalidateQueries({ queryKey: ['goals'] })
    },
  })

  const completeGoal = useMutation({
    mutationFn: (id: string) => api<GoalDto>(`/api/goals/${id}/complete`, { method: 'POST' }),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['goals'] }),
  })

  function submitGoal(event: FormEvent) {
    event.preventDefault()
    if (title.trim().length < 3) return
    createGoal.mutate()
  }

  if (roadmap.isLoading) return <LoadingBlock label="Walking the ladder…" />
  if (roadmap.isError) return <ErrorBlock message={getErrorMessage(roadmap.error)} retry={() => void roadmap.refetch()} />
  const data = roadmap.data
  if (!data) return null

  const metrics = (data.metrics ?? {}) as Record<string, number | null>
  const current = data.stages.find((stage) => stage.current)
  const next = data.stages.find((stage) => stage.target_elo === data.next_target)

  return (
    <>
      <PageHeader
        eyebrow="Report 08 · Road to 2500"
        title="Nine doors, not one leap."
        description="Milestones unlock on measured criteria. The ladder is a study plan, not a badge collection."
        actions={
          <button
            className="ef-btn ef-btn--primary"
            type="button"
            onClick={() => curriculum.refetch()}
            disabled={curriculum.isFetching}
          >
            <Sparkles size={15} />
            {curriculum.isFetching ? 'Prescribing…' : 'What should I study next?'}
          </button>
        }
      />

      <section className="roadmap-hero">
        <div className="roadmap-hero__score">
          <p className="ef-label">Current rating</p>
          <p className="roadmap-hero__number">{data.current_elo ?? '—'}</p>
          <p>{current ? `Stage ${current.stage}: ${current.title}` : 'Foundation'}</p>
        </div>
        <div className="roadmap-hero__next">
          <p className="ef-label">Next target</p>
          <p className="roadmap-hero__target ef-mono">{data.next_target ?? '—'}</p>
          <div className="progress-rule"><span style={{ width: `${data.progress_pct}%` }} /></div>
          <p className="roadmap-hero__note">
            {formatPercent(data.progress_pct, 0)} of the gap closed
            {next ? ` · ${next.description}` : ''}
          </p>
        </div>
      </section>

      {curriculum.data ? (
        <section className="curriculum">
          <div>
            <p className="ef-label">Prescribed micro-curriculum</p>
            <h2>{curriculum.data.focus}</h2>
            <p className="curriculum__text">{curriculum.data.text}</p>
          </div>
          <div className="curriculum__rule">
            <p className="ef-label">Next-week rule</p>
            <p>{curriculum.data.next_week_rule}</p>
            <span className="ef-mono">{curriculum.data.ai_status} · {curriculum.data.model}</span>
          </div>
        </section>
      ) : curriculum.error ? (
        <p className="form-error">{getErrorMessage(curriculum.error)}</p>
      ) : null}

      <div className="roadmap-layout">
        <section className="ladder-view">
          <div className="section-head">
            <div><p className="ef-label">The ladder</p><h2>Milestones and unlock criteria</h2></div>
            <span className="ef-mono">{data.stages.filter((stage) => stage.completed).length}/{data.stages.length}</span>
          </div>
          <ol className="stage-list">
            {data.stages.map((stage) => {
              const progress = (stage.progress ?? {}) as StageProgress
              return <StageRow key={stage.stage} stage={stage} criteriaMet={Boolean(progress.criteria_met)} />
            })}
          </ol>
        </section>

        <aside className="roadmap-aside">
          <section className="metric-sheet">
            <p className="ef-label">Measured inputs</p>
            <dl>
              <div><dt>Hanging rate / 10 games</dt><dd className="ef-mono">{metrics.hanging_rate_per_10_games?.toFixed(2) ?? '—'}</dd></div>
              <div><dt>Castle before move 12</dt><dd className="ef-mono">{formatPercent((metrics.castle_before_12_rate ?? 0) * 100, 0)}</dd></div>
              <div><dt>Impulsivity index</dt><dd className="ef-mono">{formatPercent((metrics.impulsivity_index ?? 0) * 100, 1)}</dd></div>
              <div><dt>Conversion rate</dt><dd className="ef-mono">{formatPercent(metrics.conversion_rate ?? 0, 0)}</dd></div>
              <div><dt>Puzzles mastered</dt><dd className="ef-mono">{metrics.puzzles_mastered ?? 0}</dd></div>
            </dl>
          </section>

          <section className="goals-panel">
            <div className="section-head">
              <div><p className="ef-label">Personal goals</p><h2>Your own commitments</h2></div>
              <span className="ef-mono">{goals.data?.length ?? 0}</span>
            </div>
            <form className="goal-form" onSubmit={submitGoal}>
              <input
                className="ef-input"
                value={title}
                onChange={(event) => setTitle(event.target.value)}
                placeholder="e.g. Play 10 games without a hanging piece"
                maxLength={120}
                aria-label="Goal title"
              />
              <div className="goal-form__row">
                <input
                  className="ef-input"
                  value={targetElo}
                  onChange={(event) => setTargetElo(event.target.value.replace(/\D/g, ''))}
                  placeholder="Target Elo (optional)"
                  inputMode="numeric"
                  aria-label="Target Elo"
                />
                <button className="ef-btn ef-btn--ghost" type="submit" disabled={createGoal.isPending || title.trim().length < 3}>
                  <Plus size={15} /> Add
                </button>
              </div>
              {createGoal.error ? <p className="form-error">{getErrorMessage(createGoal.error)}</p> : null}
            </form>

            {goals.data && goals.data.length > 0 ? (
              <ul className="goal-list">
                {goals.data.map((goal) => (
                  <li key={goal.id} className={goal.completed ? 'is-complete' : ''}>
                    <div>
                      <p>{goal.title}</p>
                      <span className="ef-mono">
                        {goal.target_elo ? `target ${goal.target_elo}` : 'habit goal'}
                      </span>
                    </div>
                    {goal.completed ? (
                      <Check size={16} />
                    ) : (
                      <button className="icon-btn" type="button" onClick={() => completeGoal.mutate(goal.id)} aria-label={`Complete ${goal.title}`}>
                        <Check size={16} />
                      </button>
                    )}
                  </li>
                ))}
              </ul>
            ) : (
              <EmptyState title="No personal goals yet">
                The ladder is shared; a personal goal is yours. Keep it small enough to finish this week.
              </EmptyState>
            )}
          </section>

          <section className="roadmap-note">
            <p className="ef-label">How completion works</p>
            <p>
              A stage unlocks at its rating target; the habit criteria stay visible as a quality check. The rating opens
              the door — the habit decides whether you keep the gains.
            </p>
          </section>
        </aside>
      </div>

      <section className="roadmap-stats">
        <StatBlock label="Current" value={data.current_elo ?? '—'} tone="brass" />
        <StatBlock label="Next target" value={data.next_target ?? '—'} />
        <StatBlock label="Stages cleared" value={data.stages.filter((stage) => stage.completed).length} tone="good" />
        <StatBlock label="Analysed games" value={metrics.analysed_games ?? 0} />
      </section>
    </>
  )
}

function StageRow({ stage, criteriaMet }: { stage: RoadmapStageView; criteriaMet: boolean }) {
  return (
    <li className={`stage-row${stage.completed ? ' is-done' : ''}${stage.current ? ' is-current' : ''}`}>
      <span className="stage-row__dot" aria-hidden="true" />
      <span className="stage-row__elo ef-mono">{stage.target_elo}</span>
      <div className="stage-row__body">
        <div className="stage-row__head">
          <h3>{stage.title}</h3>
          {stage.completed ? <span className="stage-row__state">complete</span> : stage.current ? <span className="stage-row__state">current</span> : <span className="stage-row__state">locked</span>}
        </div>
        <p>{stage.description}</p>
        <ul>
          {stage.criteria.map((criterion) => (
            <li key={criterion}>
              <span className={`stage-check${criteriaMet ? ' is-on' : ''}`} aria-hidden="true" />
              {criterion}
            </li>
          ))}
        </ul>
      </div>
    </li>
  )
}
