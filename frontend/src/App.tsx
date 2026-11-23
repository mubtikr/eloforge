import { type ReactNode } from 'react'
import { Navigate, Route, Routes } from 'react-router-dom'

import AppShell from './components/AppShell'
import { LoadingBlock } from './components/ui'
import { useAuth } from './hooks/useAuth'
import Dashboard from './pages/Dashboard'
import Focus from './pages/Focus'
import Games from './pages/Games'
import More from './pages/More'
import Landing from './pages/Landing'
import MatchLab from './pages/MatchLab'
import Openings from './pages/Openings'
import Puzzles from './pages/Puzzles'
import Roadmap from './pages/Roadmap'
import Settings from './pages/Settings'
import Sparring from './pages/Sparring'

function Protected({ children }: { children: ReactNode }) {
  const { user, isLoading } = useAuth()
  if (isLoading) {
    return (
      <div className="boot-screen">
        <LoadingBlock label="Opening your study room…" />
      </div>
    )
  }
  if (!user) return <Navigate to="/connect" replace />
  return <AppShell>{children}</AppShell>
}

export default function App() {
  return (
    <Routes>
      <Route path="/" element={<Landing />} />
      <Route path="/connect" element={<Landing />} />
      <Route path="/app" element={<Protected><Dashboard /></Protected>} />
      <Route path="/games" element={<Protected><Games /></Protected>} />
      <Route path="/games/:id" element={<Protected><MatchLab /></Protected>} />
      <Route path="/more" element={<Protected><More /></Protected>} />
      <Route path="/openings" element={<Protected><Openings /></Protected>} />
      <Route path="/focus" element={<Protected><Focus /></Protected>} />
      <Route path="/puzzles" element={<Protected><Puzzles /></Protected>} />
      <Route path="/sparring" element={<Protected><Sparring /></Protected>} />
      <Route path="/roadmap" element={<Protected><Roadmap /></Protected>} />
      <Route path="/settings" element={<Protected><Settings /></Protected>} />
      <Route path="*" element={<Navigate to="/app" replace />} />
    </Routes>
  )
}
