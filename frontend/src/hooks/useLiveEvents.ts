import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef, useState } from 'react'

import type { AppEventDto } from '../api/types'

export interface LiveState {
  connected: boolean
  progress: number | null
  message: string | null
  kind: string | null
  lastEvent: AppEventDto | null
}

export function useLiveEvents(enabled: boolean): LiveState {
  const queryClient = useQueryClient()
  const [state, setState] = useState<LiveState>({
    connected: false,
    progress: null,
    message: null,
    kind: null,
    lastEvent: null,
  })
  const refreshTimer = useRef<number | null>(null)

  useEffect(() => {
    if (!enabled) return
    const source = new EventSource('/api/events/stream')
    const named = [
      'ready',
      'sync_started',
      'sync_progress',
      'sync_game',
      'sync_done',
      'sync_error',
      'analysis_started',
      'analysis_progress',
      'analysis_done',
      'analysis_error',
      'report_ready',
    ]

    const handle = (event: MessageEvent<string>) => {
      let payload: AppEventDto | null = null
      try {
        payload = JSON.parse(event.data) as AppEventDto
      } catch {
        payload = null
      }
      setState((current) => ({
        connected: true,
        progress: payload?.progress ?? current.progress,
        message: payload?.message ?? current.message,
        kind: event.type === 'message' ? 'message' : event.type,
        lastEvent: payload ?? current.lastEvent,
      }))

      if (event.type === 'sync_done' || event.type === 'analysis_done') {
        if (refreshTimer.current !== null) window.clearTimeout(refreshTimer.current)
        refreshTimer.current = window.setTimeout(() => {
          void queryClient.invalidateQueries({ queryKey: ['games'] })
          void queryClient.invalidateQueries({ queryKey: ['game'] })
          void queryClient.invalidateQueries({ queryKey: ['sync'] })
          void queryClient.invalidateQueries({ queryKey: ['puzzles'] })
          void queryClient.invalidateQueries({ queryKey: ['roadmap'] })
          void queryClient.invalidateQueries({ queryKey: ['reports'] })
          void queryClient.invalidateQueries({ queryKey: ['focus'] })
        }, 400)
      }
    }

    const ready = (event: MessageEvent<string>) => {
      setState((current) => ({ ...current, connected: true }))
      handle(event)
    }

    source.addEventListener('ready', ready as EventListener)
    for (const name of named) {
      if (name === 'ready') continue
      source.addEventListener(name, handle as EventListener)
    }
    source.onerror = () => {
      setState((current) => ({ ...current, connected: false }))
    }

    return () => {
      if (refreshTimer.current !== null) window.clearTimeout(refreshTimer.current)
      source.close()
    }
  }, [enabled, queryClient])

  return state
}
