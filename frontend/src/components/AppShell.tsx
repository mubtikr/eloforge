import {
  BarChart3,
  BookOpen,
  CalendarCheck,
  ChevronRight,
  Crosshair,
  LayoutDashboard,
  LogOut,
  Map,
  MoreHorizontal,
  Settings,
  Shield,
  Swords,
} from 'lucide-react'
import { type ReactNode } from 'react'
import { Link, NavLink, useLocation, useNavigate } from 'react-router-dom'

import { api } from '../api/client'
import { useAuth } from '../hooks/useAuth'
import { useLiveEvents } from '../hooks/useLiveEvents'

const NAV = [
  { to: '/app', label: 'Today', icon: LayoutDashboard, end: true },
  { to: '/games', label: 'Review', icon: BookOpen },
  { to: '/openings', label: 'Openings', icon: BarChart3 },
  { to: '/focus', label: 'Focus', icon: Crosshair },
  { to: '/puzzles', label: 'Puzzles', icon: Shield },
  { to: '/sparring', label: 'Sparring', icon: Swords },
  { to: '/roadmap', label: 'Roadmap', icon: Map },
]

const MOBILE_NAV = [
  { to: '/app', label: 'Today', icon: LayoutDashboard, end: true },
  { to: '/games', label: 'Review', icon: BookOpen },
  { to: '/puzzles', label: 'Puzzles', icon: Shield },
  { to: '/sparring', label: 'Sparring', icon: Swords },
  { to: '/more', label: 'More', icon: MoreHorizontal },
]

export default function AppShell({ children }: { children: ReactNode }) {
  const { user } = useAuth()
  const live = useLiveEvents(Boolean(user))
  const navigate = useNavigate()
  const location = useLocation()

  async function logout() {
    await api<void>('/api/auth/logout', { method: 'POST' })
    navigate('/connect')
    window.location.reload()
  }

  return (
    <div className="app ef-grain">
      <a className="skip-link" href="#main-content">Skip to main content</a>
      <aside className="app-rail">
        <Link className="wordmark app-rail__brand" to="/app">
          <span className="wordmark__mark" aria-hidden="true">
            EF
          </span>
          <span className="wordmark__text">EloForge</span>
        </Link>

        <nav className="app-rail__nav" aria-label="Study sections">
          {NAV.map((item) => {
            const Icon = item.icon
            return (
              <NavLink
                key={item.to}
                to={item.to}
                end={item.end}
                className={({ isActive }) => `rail-link${isActive ? ' is-active' : ''}`}
              >
                <Icon size={17} strokeWidth={1.7} />
                {item.label}
              </NavLink>
            )
          })}
        </nav>

        <div className="app-rail__foot">
          <NavLink
            to="/settings"
            className={({ isActive }) => `rail-link${isActive ? ' is-active' : ''}`}
          >
            <Settings size={17} strokeWidth={1.7} />
            Settings
          </NavLink>
          <div className="rail-user">
            {user?.avatar_url ? (
              <img src={user.avatar_url} alt="" className="rail-user__avatar" />
            ) : (
              <span className="rail-user__avatar rail-user__avatar--fallback">
                {user?.chesscom_username.slice(0, 1).toUpperCase() ?? 'E'}
              </span>
            )}
            <div className="rail-user__meta">
              <span className="rail-user__name">{user?.display_name ?? user?.chesscom_username}</span>
              <span className="ef-mono rail-user__elo">{user?.current_elo ?? '—'}</span>
            </div>
            <button className="icon-btn" type="button" onClick={logout} aria-label="Log out">
              <LogOut size={15} />
            </button>
          </div>
          <p className="rail-disclaimer">
            Independent training tool. Not affiliated with Chess.com.
          </p>
        </div>
      </aside>

      <div className="app-body">
        <header className="app-topbar">
          <Link className="wordmark app-topbar__brand" to="/app">
            <span className="wordmark__mark" aria-hidden="true">
              EF
            </span>
            <span className="wordmark__text">EloForge</span>
          </Link>
          <div className="app-topbar__right">
            <span className={`live-dot${live.connected ? ' is-live' : ''}`} title="Live events">
              {live.connected ? 'live' : 'offline'}
            </span>
            <Link className="ef-btn ef-btn--ghost app-topbar__settings" to="/settings">
              <Settings size={15} />
              Settings
            </Link>
          </div>
        </header>

        {live.progress !== null && live.progress < 100 ? (
          <div className="live-banner" role="status">
            <div className="live-banner__track">
              <span style={{ width: `${live.progress}%` }} />
            </div>
            <span className="live-banner__text ef-mono">
              {live.kind?.replaceAll('_', ' ') ?? 'working'} · {live.progress}%
            </span>
            <span className="live-banner__message">{live.message}</span>
          </div>
        ) : null}

        <main className="app-main" id="main-content">{children}</main>

        <footer className="app-footer">
          <span>EloForge · deterministic Stockfish 17 review</span>
          <span className="app-footer__right">
            <CalendarCheck size={13} /> {user?.current_elo ? `${user.current_elo} Rapid` : 'No rating yet'}
            <ChevronRight size={13} />
            <Link to="/roadmap">Road to 2500</Link>
          </span>
        </footer>
      </div>

      <nav className="app-bottom" aria-label="Primary mobile">
        {MOBILE_NAV.map((item) => {
          const Icon = item.icon
          const moreActive =
            item.to === '/more' &&
            ['/openings', '/focus', '/roadmap', '/settings', '/more'].some(
              (path) => location.pathname === path,
            )
          return (
            <NavLink
              key={item.to}
              to={item.to}
              end={item.end}
              className={({ isActive }) =>
                `app-bottom__link${isActive || moreActive ? ' is-active' : ''}`
              }
            >
              <Icon size={18} strokeWidth={1.7} />
              <span>{item.label}</span>
            </NavLink>
          )
        })}
      </nav>
    </div>
  )
}
