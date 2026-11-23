import type { ReactNode } from 'react'

export function PageHeader({
  eyebrow,
  title,
  description,
  actions,
  className,
}: {
  eyebrow: string
  title: ReactNode
  description?: ReactNode
  actions?: ReactNode
  className?: string
}) {
  return (
    <header className={`page-head${className ? ` ${className}` : ''}`}>
      <div>
        <p className="ef-label page-head__eyebrow">{eyebrow}</p>
        <h1 className="page-head__title">{title}</h1>
        {description ? <p className="page-head__copy">{description}</p> : null}
      </div>
      {actions ? <div className="page-head__actions">{actions}</div> : null}
    </header>
  )
}

export function StatBlock({
  label,
  value,
  note,
  tone = 'ink',
}: {
  label: string
  value: ReactNode
  note?: ReactNode
  tone?: 'ink' | 'sienna' | 'brass' | 'slate' | 'good'
}) {
  return (
    <div className={`stat-block stat-block--${tone}`}>
      <p className="ef-label">{label}</p>
      <p className="stat-block__value ef-mono">{value}</p>
      {note ? <p className="stat-block__note">{note}</p> : null}
    </div>
  )
}

export function EmptyState({
  title,
  children,
  action,
}: {
  title: string
  children: ReactNode
  action?: ReactNode
}) {
  return (
    <div className="empty-state">
      <span className="empty-state__mark" aria-hidden="true">∅</span>
      <h3>{title}</h3>
      <div className="empty-state__copy">{children}</div>
      {action ? <div className="empty-state__action">{action}</div> : null}
    </div>
  )
}

export function LoadingBlock({ label = 'Loading the study room…' }: { label?: string }) {
  return (
    <div className="loading-block" role="status">
      <span className="loading-block__bar" />
      <span>{label}</span>
    </div>
  )
}

export function ErrorBlock({ message, retry }: { message: string; retry?: () => void }) {
  return (
    <div className="error-block" role="alert">
      <p>{message}</p>
      {retry ? (
        <button className="ef-btn ef-btn--ghost" type="button" onClick={retry}>
          Try again
        </button>
      ) : null}
    </div>
  )
}

export function Tag({ children, tone = 'neutral' }: { children: ReactNode; tone?: 'neutral' | 'sienna' | 'brass' | 'good' }) {
  return <span className={`tag tag--${tone}`}>{children}</span>
}

export function ResultMark({ result }: { result: string }) {
  const letter = result === 'win' ? 'W' : result === 'loss' ? 'L' : 'D'
  return <span className={`result-mark result-mark--${result}`} aria-label={result}>{letter}</span>
}
