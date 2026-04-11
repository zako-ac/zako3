import { useEffect } from 'react'
import { useQueryClient, type QueryKey } from '@tanstack/react-query'
import { AUTH_TOKEN_KEY, API_BASE_URL } from '@/lib/constants'

export const useStatsSSE = (queryKeys: QueryKey[]) => {
    const queryClient = useQueryClient()
    useEffect(() => {
        const token = localStorage.getItem(AUTH_TOKEN_KEY) ?? ''
        const url = `${API_BASE_URL}/stats/sse?token=${encodeURIComponent(token)}`
        const es = new EventSource(url)
        es.onmessage = () => {
            queryKeys.forEach((key) =>
                queryClient.invalidateQueries({ queryKey: key })
            )
        }
        es.onerror = () => es.close()
        return () => es.close()
    // queryKeys intentionally omitted — stable at call site
    // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [queryClient])
}
