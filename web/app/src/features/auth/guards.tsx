import { Navigate, useLocation } from 'react-router-dom'
import type { ReactNode } from 'react'
import { useAuthStore } from './store'
import { ROUTES } from '@/lib/constants'


interface AuthGuardProps {
  children: ReactNode
}

export const AuthGuard = ({ children }: AuthGuardProps) => {
  const { isAuthenticated } = useAuthStore()
  const location = useLocation()

  if (!isAuthenticated) {
    return <Navigate to={ROUTES.LOGIN} state={{ from: location }} replace />
  }

  return <>{children}</>
}

interface AdminGuardProps {
  children: ReactNode
}

export const AdminGuard = ({ children }: AdminGuardProps) => {
  const { isAuthenticated, user } = useAuthStore()
  const location = useLocation()

  if (!isAuthenticated) {
    return <Navigate to={ROUTES.LOGIN} state={{ from: location }} replace />
  }

  if (!user?.isAdmin) {
    return <Navigate to={ROUTES.DASHBOARD} replace />
  }

  return <>{children}</>
}

interface GuestGuardProps {
  children: ReactNode
}

export const GuestGuard = ({ children }: GuestGuardProps) => {
  const { isAuthenticated } = useAuthStore()
  const location = useLocation()

  if (isAuthenticated) {
    const from = (location.state as { from?: Location })?.from?.pathname || ROUTES.DASHBOARD
    return <Navigate to={from} replace />
  }

  return <>{children}</>
}
