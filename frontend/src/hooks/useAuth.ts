import { useQuery } from '@tanstack/react-query'

import { api } from '../api/client'
import type { AuthResponse } from '../api/types'

export function useAuth() {
  const query = useQuery({
    queryKey: ['me'],
    queryFn: () => api<AuthResponse>('/api/auth/me'),
    retry: false,
    staleTime: 60_000,
  })
  return {
    user: query.data?.user ?? null,
    isLoading: query.isLoading,
    isError: query.isError,
    refetch: query.refetch,
  }
}
