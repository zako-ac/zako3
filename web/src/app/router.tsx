import { Routes, Route, Navigate } from 'react-router-dom'
import { RootLayout, AppLayout, AdminLayout, AuthLayout } from '@/layouts'
import { AuthGuard, AdminGuard, GuestGuard } from '@/features/auth'
import {
  LoginPage,
  AuthCallbackPage,
  DashboardPage,
  TapExplorePage,
  MyTapsPage,
  CreateTapPage,
  TapSettingsPage,
  TapStatsPage,
  SettingsPage,
  AdminDashboardPage,
  AdminUsersPage,
  AdminUserDetailPage,
  AdminTapsPage,
  AdminTapDetailPage,
  AdminNotificationsPage,
  AdminVerificationsPage,
} from '@/pages'
import { HotkeyTest } from '@/pages/hotkey-test'
import { ROUTES } from '@/lib/constants'

export const AppRouter = () => {
  return (
    <Routes>
      <Route element={<RootLayout />}>
        {/* Auth routes */}
        <Route element={<AuthLayout />}>
          <Route
            path={ROUTES.LOGIN}
            element={
              <GuestGuard>
                <LoginPage />
              </GuestGuard>
            }
          />
        </Route>
        <Route path={ROUTES.AUTH_CALLBACK} element={<AuthCallbackPage />} />

        {/* Public app routes */}
        <Route element={<AppLayout />}>
          <Route path={ROUTES.TAPS} element={<TapExplorePage />} />
          <Route path="/taps/:tapId/stats" element={<TapStatsPage />} />
        </Route>

        {/* App routes (authenticated) */}
        <Route
          element={
            <AuthGuard>
              <AppLayout />
            </AuthGuard>
          }
        >
          <Route path={ROUTES.DASHBOARD} element={<DashboardPage />} />
          <Route path={ROUTES.SETTINGS} element={<SettingsPage />} />
          <Route path={ROUTES.TAPS_MINE} element={<MyTapsPage />} />
          <Route path={ROUTES.TAPS_CREATE} element={<CreateTapPage />} />
          <Route path="/taps/:tapId/settings" element={<TapSettingsPage />} />
        </Route>

        {/* Admin routes */}
        <Route
          element={
            <AdminGuard>
              <AdminLayout />
            </AdminGuard>
          }
        >
          <Route path={ROUTES.ADMIN} element={<AdminDashboardPage />} />
          <Route path={ROUTES.ADMIN_USERS} element={<AdminUsersPage />} />
          <Route
            path="/admin/users/:userId"
            element={<AdminUserDetailPage />}
          />
          <Route path={ROUTES.ADMIN_TAPS} element={<AdminTapsPage />} />
          <Route path="/admin/taps/:tapId" element={<AdminTapDetailPage />} />
          <Route
            path={ROUTES.ADMIN_NOTIFICATIONS}
            element={<AdminNotificationsPage />}
          />
          <Route
            path={ROUTES.ADMIN_VERIFICATIONS}
            element={<AdminVerificationsPage />}
          />
        </Route>

        {/* Redirects */}
        <Route path="/hotkey-test" element={<HotkeyTest />} />
        <Route
          path={ROUTES.HOME}
          element={<Navigate to={ROUTES.DASHBOARD} replace />}
        />
        <Route path="*" element={<Navigate to={ROUTES.DASHBOARD} replace />} />
      </Route>
    </Routes>
  )
}
