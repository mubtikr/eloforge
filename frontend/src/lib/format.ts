import { getDisplayTimeZone } from './timezone'

export function formatEval(cp: number | null | undefined, mate?: number | null): string {
  if (mate !== null && mate !== undefined) {
    return mate > 0 ? `M${mate}` : `-M${Math.abs(mate)}`
  }
  if (cp === null || cp === undefined) return '—'
  const pawns = cp / 100
  return `${pawns >= 0 ? '+' : ''}${pawns.toFixed(2)}`
}

export function winPercent(cp: number | null | undefined): number {
  if (cp === null || cp === undefined) return 50
  return 50 + 50 * (2 / (1 + Math.exp(-0.00368208 * cp)) - 1)
}

export function formatPercent(value: number | null | undefined, digits = 0): string {
  if (value === null || value === undefined || Number.isNaN(value)) return '—'
  return `${value.toFixed(digits)}%`
}

export function formatClock(seconds: number | null | undefined): string {
  if (seconds === null || seconds === undefined) return '—'
  const total = Math.max(0, Math.round(seconds))
  const h = Math.floor(total / 3600)
  const m = Math.floor((total % 3600) / 60)
  const s = total % 60
  if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
  return `${m}:${String(s).padStart(2, '0')}`
}

export function formatDate(value: string | null | undefined): string {
  if (!value) return '—'
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return '—'
  return new Intl.DateTimeFormat('en-GB', {
    day: '2-digit',
    month: 'short',
    year: 'numeric',
    timeZone: getDisplayTimeZone(),
  }).format(date)
}

export function formatDateTime(value: string | null | undefined): string {
  if (!value) return '—'
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return '—'
  return new Intl.DateTimeFormat('en-GB', {
    day: '2-digit',
    month: 'short',
    hour: '2-digit',
    minute: '2-digit',
    timeZone: getDisplayTimeZone(),
  }).format(date)
}

export function formatWeekdayDate(value: Date): string {
  return new Intl.DateTimeFormat('en-GB', {
    weekday: 'long',
    day: '2-digit',
    month: 'long',
    timeZone: getDisplayTimeZone(),
  }).format(value)
}

export function relativeTime(value: string | null | undefined): string {
  if (!value) return 'never'
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return 'never'
  const diff = Date.now() - date.getTime()
  const minutes = Math.round(diff / 60_000)
  if (minutes < 1) return 'just now'
  if (minutes < 60) return `${minutes}m ago`
  const hours = Math.round(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  const days = Math.round(hours / 24)
  if (days < 30) return `${days}d ago`
  return formatDate(value)
}

export function formatRelativeRule(value: string | null | undefined): string {
  return relativeTime(value)
}

export function classificationLabel(value: string | null | undefined): string {
  switch (value) {
    case 'best':
      return 'Best'
    case 'strong':
      return 'Strong'
    case 'good':
      return 'Good'
    case 'inaccuracy':
      return 'Inaccuracy'
    case 'mistake':
      return 'Mistake'
    case 'blunder':
      return 'Blunder'
    case 'missed_win':
      return 'Missed win'
    default:
      return 'Unclassified'
  }
}

export function classificationClass(value: string | null | undefined): string {
  switch (value) {
    case 'best':
    case 'strong':
    case 'good':
      return 'is-good'
    case 'inaccuracy':
      return 'is-inaccuracy'
    case 'mistake':
      return 'is-mistake'
    case 'blunder':
    case 'missed_win':
      return 'is-blunder'
    default:
      return ''
  }
}

export function tagLabel(tag: string): string {
  return tag
    .toLowerCase()
    .split('_')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ')
}

export function resultLabel(result: string): string {
  if (result === 'win') return 'Win'
  if (result === 'loss') return 'Loss'
  if (result === 'draw') return 'Draw'
  return result
}

export function uciToSquares(uci: string | null | undefined): [string, string] | null {
  if (!uci || uci.length < 4) return null
  return [uci.slice(0, 2), uci.slice(2, 4)]
}

export function shortName(value: string, max = 18): string {
  return value.length > max ? `${value.slice(0, max - 1)}…` : value
}
