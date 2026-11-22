import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Bell, LogOut, Send } from 'lucide-react'
import { useState, type FormEvent } from 'react'
import { useNavigate } from 'react-router-dom'

import { api, getErrorMessage } from '../api/client'
import type {
  NotificationDelivery,
  NotificationSettingsDto,
  VapidKeyResponse,
} from '../api/types'
import { ErrorBlock, LoadingBlock, PageHeader } from '../components/ui'
import { useAuth } from '../hooks/useAuth'

const TIMEZONES = [
  'UTC',
  'Europe/London',
  'Europe/Berlin',
  'Europe/Madrid',
  'Europe/Paris',
  'Europe/Istanbul',
  'America/New_York',
  'America/Chicago',
  'America/Los_Angeles',
  'Asia/Dubai',
  'Asia/Kolkata',
  'Asia/Singapore',
  'Australia/Sydney',
]

function urlBase64ToUint8Array(base64: string): Uint8Array<ArrayBuffer> {
  const padding = '='.repeat((4 - (base64.length % 4)) % 4)
  const normalized = (base64 + padding).replace(/-/g, '+').replace(/_/g, '/')
  const raw = window.atob(normalized)
  const bytes = new Uint8Array(new ArrayBuffer(raw.length))
  for (let index = 0; index < raw.length; index += 1) bytes[index] = raw.charCodeAt(index)
  return bytes
}

export default function Settings() {
  const { user } = useAuth()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const [draft, setDraft] = useState<NotificationSettingsDto | null>(null)
  const [deliveries, setDeliveries] = useState<NotificationDelivery[] | null>(null)
  const [pushState, setPushState] = useState<string | null>(null)

  const settings = useQuery({
    queryKey: ['settings', 'notifications'],
    queryFn: () => api<NotificationSettingsDto>('/api/settings/notifications'),
  })
  const vapid = useQuery({
    queryKey: ['settings', 'vapid'],
    queryFn: () => api<VapidKeyResponse>('/api/settings/vapid'),
  })
  const health = useQuery({
    queryKey: ['health'],
    queryFn: () => api<{ engine: { model: string; protocol: string } }>('/api/healthz'),
    retry: false,
  })

  const save = useMutation({
    mutationFn: (payload: NotificationSettingsDto) =>
      api<NotificationSettingsDto>('/api/settings/notifications', {
        method: 'PUT',
        body: JSON.stringify({
          telegram_chat_id: payload.telegram_chat_id,
          email: payload.email,
          web_push_subscription: payload.web_push_subscription,
          daily_reminder: payload.daily_reminder,
          tilt_alerts: payload.tilt_alerts,
          reminder_hour: payload.reminder_hour,
          timezone: payload.timezone,
        }),
      }),
    onSuccess: (data) => {
      setDraft(data)
      void queryClient.invalidateQueries({ queryKey: ['settings', 'notifications'] })
    },
  })

  const test = useMutation({
    mutationFn: () => api<NotificationDelivery[]>('/api/settings/notifications/test', { method: 'POST' }),
    onSuccess: (data) => setDeliveries(data),
  })

  async function enablePush() {
    if (!vapid.data?.public_key) {
      setPushState('VAPID_PUBLIC_KEY is not configured on the server.')
      return
    }
    if (!('serviceWorker' in navigator) || !('PushManager' in window)) {
      setPushState('This browser does not support Web Push.')
      return
    }
    try {
      const registration = await navigator.serviceWorker.register('/sw.js')
      const permission = await Notification.requestPermission()
      if (permission !== 'granted') {
        setPushState('Notification permission was not granted.')
        return
      }
      const existing = await registration.pushManager.getSubscription()
      const subscription =
        existing ??
        (await registration.pushManager.subscribe({
          userVisibleOnly: true,
          applicationServerKey: urlBase64ToUint8Array(vapid.data.public_key),
        }))
      const base = draft ?? settings.data
      if (base) {
        setDraft({ ...base, web_push_subscription: subscription.toJSON() })
        setPushState('Subscription captured. Save the settings to store it.')
      }
    } catch (err) {
      setPushState(err instanceof Error ? err.message : 'Could not subscribe to push.')
    }
  }

  function submit(event: FormEvent) {
    event.preventDefault()
    if (form) save.mutate(form)
  }

  async function logout() {
    await api<void>('/api/auth/logout', { method: 'POST' })
    navigate('/connect')
    window.location.reload()
  }

  if (settings.isLoading) return <LoadingBlock label="Loading notification settings…" />
  if (settings.isError) return <ErrorBlock message={getErrorMessage(settings.error)} retry={() => void settings.refetch()} />
  const form = draft ?? settings.data
  if (!form) return null

  return (
    <>
      <PageHeader
        eyebrow="Settings"
        title="Quiet study, timely nudges."
        description="Reminders should arrive when they help. Every channel is optional; nothing is sent without a saved setting."
      />

      <div className="settings-layout">
        <form className="settings-form" onSubmit={submit}>
          <section className="settings-section">
            <div className="section-head">
              <div><p className="ef-label">Account</p><h2>{user?.display_name ?? user?.chesscom_username}</h2></div>
              <span className="ef-mono">Rapid {user?.current_elo ?? '—'}</span>
            </div>
            <dl className="account-facts">
              <div><dt>Chess.com</dt><dd>{user?.chesscom_username}</dd></div>
              <div><dt>Peak</dt><dd className="ef-mono">{user?.peak_elo ?? '—'}</dd></div>
              <div><dt>Country</dt><dd>{user?.country?.split('/').pop() ?? '—'}</dd></div>
              <div><dt>Coach model</dt><dd className="ef-mono">{health.data?.engine.model ?? 'deepseek-v4.1-flash'}</dd></div>
            </dl>
            <button className="ef-btn ef-btn--ghost" type="button" onClick={logout}>
              <LogOut size={15} /> Log out
            </button>
          </section>

          <section className="settings-section">
            <p className="ef-label">Reminders</p>
            <h2>Daily rhythm</h2>
            <div className="settings-row">
              <label className="switch">
                <input
                  type="checkbox"
                  checked={form.daily_reminder}
                  onChange={(event) => setDraft({ ...form, daily_reminder: event.target.checked })}
                />
                <span>Daily study nudge</span>
              </label>
              <label className="switch">
                <input
                  type="checkbox"
                  checked={form.tilt_alerts}
                  onChange={(event) => setDraft({ ...form, tilt_alerts: event.target.checked })}
                />
                <span>Tilt and loss-streak warning</span>
              </label>
            </div>
            <div className="settings-row">
              <label className="settings-field">
                <span className="ef-label">Reminder hour</span>
                <select
                  className="ef-input"
                  value={form.reminder_hour}
                  onChange={(event) => setDraft({ ...form, reminder_hour: Number(event.target.value) })}
                >
                  {Array.from({ length: 24 }, (_, hour) => (
                    <option key={hour} value={hour}>
                      {String(hour).padStart(2, '0')}:00
                    </option>
                  ))}
                </select>
              </label>
              <label className="settings-field">
                <span className="ef-label">Timezone</span>
                <select
                  className="ef-input"
                  value={form.timezone}
                  onChange={(event) => setDraft({ ...form, timezone: event.target.value })}
                >
                  {TIMEZONES.map((timezone) => (
                    <option key={timezone} value={timezone}>{timezone}</option>
                  ))}
                </select>
              </label>
            </div>
          </section>

          <section className="settings-section">
            <p className="ef-label">Channels</p>
            <h2>Where the nudge lands</h2>
            <label className="settings-field">
              <span className="ef-label">Telegram chat ID</span>
              <input
                className="ef-input"
                value={form.telegram_chat_id ?? ''}
                onChange={(event) => setDraft({ ...form, telegram_chat_id: event.target.value || null })}
                placeholder="e.g. 123456789"
                inputMode="numeric"
              />
            </label>
            <label className="settings-field">
              <span className="ef-label">Email</span>
              <input
                className="ef-input"
                type="email"
                value={form.email ?? ''}
                onChange={(event) => setDraft({ ...form, email: event.target.value || null })}
                placeholder="you@example.com"
              />
            </label>
            <div className="push-block">
              <div>
                <p className="ef-label">Web Push</p>
                <p>{form.web_push_subscription ? 'A browser subscription is stored.' : 'No browser subscription yet.'}</p>
                {pushState ? <p className="push-block__state">{pushState}</p> : null}
              </div>
              <button className="ef-btn ef-btn--ghost" type="button" onClick={() => void enablePush()}>
                <Bell size={15} /> {form.web_push_subscription ? 'Re-subscribe' : 'Enable Web Push'}
              </button>
            </div>
          </section>

          <div className="settings-actions">
            <button className="ef-btn ef-btn--primary" type="submit" disabled={save.isPending}>
              {save.isPending ? 'Saving…' : 'Save settings'}
            </button>
            <button
              className="ef-btn ef-btn--ghost"
              type="button"
              onClick={() => test.mutate()}
              disabled={test.isPending}
            >
              <Send size={15} /> {test.isPending ? 'Sending…' : 'Send test'}
            </button>
            {save.error ? <p className="form-error">{getErrorMessage(save.error)}</p> : null}
            {test.error ? <p className="form-error">{getErrorMessage(test.error)}</p> : null}
          </div>
          {deliveries ? (
            <ul className="delivery-list">
              {deliveries.map((delivery) => (
                <li key={delivery.channel}>
                  <span className={`delivery-dot${delivery.ok ? ' is-ok' : ''}`} />
                  <strong>{delivery.channel}</strong>
                  <span>{delivery.detail}</span>
                </li>
              ))}
            </ul>
          ) : null}
        </form>

        <aside className="settings-aside">
          <section>
            <p className="ef-label">Privacy</p>
            <h3>What leaves this server</h3>
            <p>
              Chess.com sync is read-only. The coach sends only verified engine facts and your reflection text to OpenCode Go.
              API keys stay in the server environment and never reach the browser.
            </p>
          </section>
          <section>
            <p className="ef-label">AI budget</p>
            <h3>Daily cap</h3>
            <p>
              Coaching calls count against a server-side daily token budget. When the budget is spent, reflections fall back
              to deterministic engine notes with the same next-time rule.
            </p>
          </section>
          <section>
            <p className="ef-label">Non-affiliation</p>
            <p>
              Independent training tool. Not affiliated with or endorsed by Chess.com. Board colours, pieces and feature
              names are original or licensed.
            </p>
          </section>
        </aside>
      </div>
    </>
  )
}
