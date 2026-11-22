import { RefreshCw } from 'lucide-react'
import { useMutation, useQueryClient } from '@tanstack/react-query'

import { api, getErrorMessage } from '../api/client'
import type { SyncTriggerResponse } from '../api/types'

export default function SyncButton({ compact = false }: { compact?: boolean }) {
  const queryClient = useQueryClient()
  const mutation = useMutation({
    mutationFn: () => api<SyncTriggerResponse>('/api/sync/trigger', { method: 'POST' }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['sync'] })
      void queryClient.invalidateQueries({ queryKey: ['games'] })
    },
  })

  return (
    <div className="sync-button">
      <button
        className={`ef-btn ${compact ? 'ef-btn--ghost' : 'ef-btn--primary'}`}
        type="button"
        onClick={() => mutation.mutate()}
        disabled={mutation.isPending}
      >
        <RefreshCw size={15} className={mutation.isPending ? 'spin' : undefined} />
        {mutation.isPending ? 'Starting sync…' : 'Sync Chess.com'}
      </button>
      {mutation.error ? <span className="sync-button__error">{getErrorMessage(mutation.error)}</span> : null}
      {mutation.data && !mutation.data.accepted ? (
        <span className="sync-button__note">{mutation.data.message}</span>
      ) : null}
    </div>
  )
}
