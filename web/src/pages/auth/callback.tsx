import { useEffect, useRef } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { useAuthCallback } from '@/features/auth/hooks'
import { ROUTES } from '@/lib/constants'
import { Spinner } from '@/components/ui/spinner'
import { toast } from 'sonner'

export const AuthCallbackPage = () => {
  const navigate = useNavigate()
  const [searchParams] = useSearchParams()
  const { mutate: handleCallback } = useAuthCallback()
  const hasAttempted = useRef(false)

  useEffect(() => {
    if (hasAttempted.current) return
    hasAttempted.current = true

    const code = searchParams.get('code')
    const state = searchParams.get('state')
    const error = searchParams.get('error')

    if (error) {
      console.error('OAuth error:', error)
      toast.error('Authentication failed')
      navigate(ROUTES.LOGIN, { replace: true })
      return
    }

    if (code) {
      handleCallback({ code, state }, {
        onError: () => {
          toast.error('Failed to verify authentication code')
          navigate(ROUTES.LOGIN, { replace: true })
        }
      })
    } else {
      navigate(ROUTES.LOGIN, { replace: true })
    }
  }, [searchParams, navigate, handleCallback])

  return (
    <div className="flex min-h-svh items-center justify-center">
      <div className="flex flex-col items-center gap-4">
        <Spinner className="h-8 w-8" />
        <p className="text-muted-foreground">Authenticating...</p>
      </div>
    </div>
  )
}
