import { useEffect } from 'react'
import { useQueryClient, type QueryKey } from '@tanstack/react-query'
import { fetchEventSource } from '@microsoft/fetch-event-source'
import { AUTH_TOKEN_KEY, API_BASE_URL } from '@/lib/constants'

export const useStatsSSE = (queryKeys: QueryKey[]) => {
    const queryClient = useQueryClient()
    useEffect(() => {
        const token = localStorage.getItem(AUTH_TOKEN_KEY) ?? ''
        const ctrl = new AbortController()
        fetchEventSource(`${API_BASE_URL}/stats/sse`, {
            headers: { Authorization: `Bearer ${token}` },
            signal: ctrl.signal,
            onmessage() {
                queryKeys.forEach((key) =>
                    queryClient.invalidateQueries({ queryKey: key })
                )
            },
            onerror() {
                ctrl.abort()
            },
        })
        return () => ctrl.abort()
    // queryKeys intentionally omitted — stable at call site
    // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [queryClient])
}
