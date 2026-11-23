import { BarChart3, Crosshair, Map, Settings, Shield, Swords } from 'lucide-react'
import { Link } from 'react-router-dom'

import { PageHeader } from '../components/ui'

const LINKS = [
  { to: '/openings', title: 'Openings', copy: 'Where your repertoire holds and where it leaks.', icon: BarChart3 },
  { to: '/focus', title: 'Focus & tilt', copy: 'Clock habits, session fatigue and your best hours.', icon: Crosshair },
  { to: '/roadmap', title: 'Road to 2500', copy: 'Milestones, habit checks, goals and the next study block.', icon: Map },
  { to: '/puzzles', title: 'Puzzles', copy: 'Personal blunder drills scheduled by FSRS.', icon: Shield },
  { to: '/sparring', title: 'Endgame Arena', copy: 'Convert the winning endgames you threw away.', icon: Swords },
  { to: '/settings', title: 'Settings', copy: 'Timezone, reminders, passwordless session and coach mode.', icon: Settings },
]

export default function More() {
  return (
    <>
      <PageHeader
        eyebrow="More"
        title="Everything else in the study room."
        description="The five-item mobile bar keeps the daily loop visible. Your reports and settings live here."
      />
      <div className="more-grid">
        {LINKS.map((item) => {
          const Icon = item.icon
          return (
            <Link key={item.to} className="more-link" to={item.to}>
              <Icon size={20} strokeWidth={1.6} />
              <strong>{item.title}</strong>
              <span>{item.copy}</span>
            </Link>
          )
        })}
      </div>
    </>
  )
}
