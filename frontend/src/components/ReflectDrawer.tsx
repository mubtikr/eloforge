import { X } from 'lucide-react'
import { useEffect, useRef, useState, type FormEvent } from 'react'

import type { MoveDto, ReflectDone, ReflectMeta } from '../api/types'
import { getErrorMessage } from '../api/client'
import { streamSse } from '../api/sse'
import { classificationLabel, formatEval, formatPercent } from '../lib/format'

const QUICK_TAGS = [
  { id: 'didnt_see_defender', label: "Didn't see the defender" },
  { id: 'tunnel_vision', label: 'Tunnel vision on my attack' },
  { id: 'clock_panic', label: 'Panicked on the clock' },
  { id: 'forgot_pattern', label: 'Forgot the pattern' },
  { id: 'overconfidence', label: 'Overconfident' },
  { id: 'opening_autopilot', label: 'Opening autopilot' },
] as const

interface ReflectDrawerProps {
  open: boolean
  gameId: string
  move: MoveDto | null
  onClose: () => void
  onSaved: () => void
}

export default function ReflectDrawer({ open, gameId, move, onClose, onSaved }: ReflectDrawerProps) {
  const [tags, setTags] = useState<string[]>([])
  const [explanation, setExplanation] = useState('')
  const [feedback, setFeedback] = useState('')
  const [rule, setRule] = useState('')
  const [status, setStatus] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [meta, setMeta] = useState<ReflectMeta | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [streaming, setStreaming] = useState(false)
  const abortRef = useRef<AbortController | null>(null)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  useEffect(() => {
    if (!open) return
    const timer = window.setTimeout(() => textareaRef.current?.focus(), 80)
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', escape)
    return () => {
      window.clearTimeout(timer)
      window.removeEventListener('keydown', escape)
      abortRef.current?.abort()
    }
  }, [open, onClose])

  if (!open || !move) return null

  function toggleTag(id: string) {
    setTags((current) =>
      current.includes(id) ? current.filter((tag) => tag !== id) : [...current, id].slice(0, 5),
    )
  }

  async function submit(event: FormEvent) {
    event.preventDefault()
    if (!move || explanation.trim().length < 3) return
    setStreaming(true)
    setError(null)
    setFeedback('')
    setRule('')
    setNotice(null)
    const controller = new AbortController()
    abortRef.current = controller
    try {
      await streamSse(
        `/api/games/${gameId}/moves/${move.id}/reflect`,
        { reason_tags: tags, explanation: explanation.trim() },
        (frame) => {
          if (frame.event === 'meta') setMeta(frame.data as ReflectMeta)
          if (frame.event === 'notice') {
            const payload = frame.data as { message?: string }
            setNotice(payload.message ?? null)
          }
          if (frame.event === 'delta') {
            const payload = frame.data as { text?: string }
            if (payload.text) setFeedback((current) => current + payload.text)
          }
          if (frame.event === 'done') {
            const payload = frame.data as ReflectDone
            setRule(payload.next_time_rule)
            setStatus(payload.ai_status)
            setStreaming(false)
            onSaved()
          }
        },
        controller.signal,
      )
    } catch (err) {
      if (!controller.signal.aborted) {
        setError(getErrorMessage(err))
        setStreaming(false)
      }
    }
  }

  return (
    <div className="drawer-layer" role="presentation">
      <button className="drawer-backdrop" type="button" aria-label="Close reflection" onClick={onClose} />
      <aside className="drawer" role="dialog" aria-modal="true" aria-label="Why did I play this?">
        <header className="drawer__head">
          <div>
            <p className="ef-label">Reflection</p>
            <h2>Why did I play this?</h2>
          </div>
          <button className="icon-btn" type="button" onClick={onClose} aria-label="Close">
            <X size={17} />
          </button>
        </header>

        <div className="drawer__move">
          <span className="ef-mono">
            {move.move_number}. {move.san}
          </span>
          <span className={`class-pill ${move.classification === 'missed_win' ? 'is-blunder' : `is-${move.classification ?? 'unknown'}`}`}>
            {classificationLabel(move.classification)}
          </span>
          <span className="ef-mono drawer__eval">
            {formatEval(move.eval_after_cp, move.eval_mate)}
          </span>
          {move.epl !== null ? (
            <span className="drawer__epl ef-mono">−{formatPercent(move.epl * 100, 1)} win</span>
          ) : null}
        </div>
        {move.best_move_san ? (
          <p className="drawer__best">
            Best was <span className="ef-mono">{move.best_move_san}</span>.
          </p>
        ) : null}

        <form className="drawer__form" onSubmit={submit}>
          <fieldset className="drawer__tags" disabled={streaming}>
            <legend className="ef-label">What were you thinking?</legend>
            <div className="tag-cloud">
              {QUICK_TAGS.map((tag) => (
                <button
                  key={tag.id}
                  type="button"
                  className={`tag-toggle${tags.includes(tag.id) ? ' is-on' : ''}`}
                  onClick={() => toggleTag(tag.id)}
                  aria-pressed={tags.includes(tag.id)}
                >
                  {tag.label}
                </button>
              ))}
            </div>
          </fieldset>
          <label className="drawer__field">
            <span className="ef-label">In your own words</span>
            <textarea
              ref={textareaRef}
              className="ef-input drawer__textarea"
              value={explanation}
              onChange={(event) => setExplanation(event.target.value)}
              placeholder="I was looking at my own attack and never checked what recaptured…"
              maxLength={2000}
              rows={4}
              disabled={streaming}
            />
            <span className="drawer__counter ef-mono">{explanation.length}/2000</span>
          </label>
          <button
            className="ef-btn ef-btn--primary drawer__submit"
            type="submit"
            disabled={streaming || explanation.trim().length < 3}
          >
            {streaming ? 'Coach is reading the position…' : 'Ask the coach'}
          </button>
        </form>

        {error ? <p className="drawer__error">{error}</p> : null}
        {notice ? <p className="drawer__notice">{notice}</p> : null}

        {feedback || streaming ? (
          <div className="drawer__response" aria-live="polite">
            <p className="ef-label">
              {meta?.ai_configured ? `Coach · ${meta.model}` : 'Deterministic coach notes'}
            </p>
            <p className="drawer__feedback">{feedback || '…'}</p>
            {streaming ? <span className="drawer__cursor" aria-hidden="true" /> : null}
          </div>
        ) : null}

        {rule ? (
          <div className="next-rule">
            <p className="ef-label">Next-time rule</p>
            <p>{rule}</p>
            {status && status !== 'ok' ? (
              <span className="next-rule__status ef-mono">
                {status === 'unavailable' ? 'AI key not configured' : `fallback: ${status}`}
              </span>
            ) : null}
          </div>
        ) : null}
      </aside>
    </div>
  )
}
