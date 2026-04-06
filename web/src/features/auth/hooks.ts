import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { authApi } from './api'
import { useAuthStore } from './store'
import { ROUTES } from '@/lib/constants'


export const authKeys = {
  all: ['auth'] as const,
  user: () => [...authKeys.all, 'user'] as const,
}

export const useCurrentUser = () => {
  const { isAuthenticated, setUser } = useAuthStore()

  return useQuery({
    queryKey: authKeys.user(),
    queryFn: async () => {
      const user = await authApi.getCurrentUser()
      setUser(user)
      return user
    },
    enabled: isAuthenticated,
    staleTime: 1000 * 60 * 5,
  })
}

export const useLogin = () => {
  return (redirect?: string) => {
    const url = redirect
      ? `/api/v1/auth/login?redirect=${encodeURIComponent(redirect)}`
      : '/api/v1/auth/login'
    window.location.href = url
  }
}

export const useAuthCallback = () => {
  const { login } = useAuthStore()
  const navigate = useNavigate()
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: ({ code, state }: { code: string; state: string | null }) =>
      authApi.handleCallback(code, state),
    onSuccess: (data) => {
      login(data.token, data.user)
      queryClient.setQueryData(authKeys.user(), data.user)
      navigate(data.redirectUrl ?? ROUTES.DASHBOARD)
    },
  })
}

export const useLogout = () => {
  const { logout } = useAuthStore()
  const navigate = useNavigate()
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: authApi.logout,
    onSuccess: () => {
      logout()
      queryClient.clear()
      navigate(ROUTES.LOGIN)
    },
  })
}
