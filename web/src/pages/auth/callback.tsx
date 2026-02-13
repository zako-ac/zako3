import { useEffect } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { useAuthStore } from '@/features/auth'
import { ROUTES } from '@/lib/constants'
import { Spinner } from '@/components/ui/spinner'

export const AuthCallbackPage = () => {
  const navigate = useNavigate()
  const [searchParams] = useSearchParams()
  const { login } = useAuthStore()

  useEffect(() => {
    const code = searchParams.get('code')
    const error = searchParams.get('error')

    if (error) {
      console.error('OAuth error:', error)
      navigate(ROUTES.LOGIN, { replace: true })
      return
    }

    if (code) {
      // In a real app, we'd exchange the code for tokens via API
      // For now with MSW, we simulate this by calling the mock endpoint
      fetch('/api/auth/callback', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ code }),
      })
        .then((res) => res.json())
        .then((data) => {
          if (data.user && data.token) {
            login(data.token, data.user)
            navigate(ROUTES.DASHBOARD, { replace: true })
          } else {
            navigate(ROUTES.LOGIN, { replace: true })
          }
        })
        .catch(() => {
          navigate(ROUTES.LOGIN, { replace: true })
        })
    } else {
      navigate(ROUTES.LOGIN, { replace: true })
    }
  }, [searchParams, navigate, login])

  return (
    <div className="flex min-h-svh items-center justify-center">
      <div className="flex flex-col items-center gap-4">
        <Spinner className="h-8 w-8" />
        <p className="text-muted-foreground">Authenticating...</p>
      </div>
    </div>
  )
}
