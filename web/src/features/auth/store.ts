import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import type { AuthUser, AuthState } from '@zako-ac/zako3-data'
import { AUTH_TOKEN_KEY, AUTH_USER_KEY } from '@/lib/constants'

interface AuthStore extends AuthState {
  login: (token: string, user: AuthUser) => void
  logout: () => void
  setUser: (user: AuthUser) => void
}

export const useAuthStore = create<AuthStore>()(
  persist(
    (set) => ({
      isAuthenticated: false,
      user: null,
      token: null,

      login: (token, user) => {
        localStorage.setItem(AUTH_TOKEN_KEY, token)
        set({ isAuthenticated: true, token, user })
      },

      logout: () => {
        localStorage.removeItem(AUTH_TOKEN_KEY)
        set({ isAuthenticated: false, token: null, user: null })
      },

      setUser: (user) => {
        set({ user })
      },
    }),
    {
      name: AUTH_USER_KEY,
      partialize: (state) => ({
        isAuthenticated: state.isAuthenticated,
        user: state.user,
        token: state.token,
      }),
    }
  )
)
