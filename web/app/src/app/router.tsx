import { Routes, Route, Navigate } from 'react-router-dom'
import { RootLayout, AppLayout, AdminLayout, AuthLayout, TapLayout } from '@/layouts'
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
  TapApiKeysPage,
  TapVerificationPage,
  SettingsPage,
  AdminDashboardPage,
  AdminUsersPage,
  AdminUserDetailPage,
  AdminUserSettingsPage,
  AdminUserSettingsUserPage,
  AdminUserSettingsGuildUserPage,
  AdminTapsPage,
  AdminTapDetailPage,
  AdminNotificationsPage,
  AdminVerificationsPage,
  AdminGlobalSettingsPage,
  AdminMappersPage,
  AdminMappersPipelinePage,
  VoiceChannelPage,
  GuildSettingsPage,
  GuildMySettingsPage,
  GuildGuildSettingsPage,
  NotFoundPage,
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
          <Route element={<TapLayout />}>
            <Route path="/taps/:tapId/stats" element={<TapStatsPage />} />
          </Route>
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
          <Route path="/voice/:guildId/:channelId" element={<VoiceChannelPage />} />
          <Route path="/guilds/:guildId/settings" element={<GuildSettingsPage />}>
            <Route index element={<Navigate to="me" replace />} />
            <Route path="me" element={<GuildMySettingsPage />} />
            <Route path="guild" element={<GuildGuildSettingsPage />} />
          </Route>
          <Route element={<TapLayout />}>
            <Route path="/taps/:tapId/settings" element={<TapSettingsPage />} />
            <Route path="/taps/:tapId/api-keys" element={<TapApiKeysPage />} />
            <Route path="/taps/:tapId/verification" element={<TapVerificationPage />} />
          </Route>
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
          <Route path="/admin/users/:userId/settings" element={<AdminUserSettingsPage />}>
            <Route index element={<Navigate to="user" replace />} />
            <Route path="user" element={<AdminUserSettingsUserPage />} />
            <Route path="guild-user" element={<AdminUserSettingsGuildUserPage />} />
          </Route>
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
          <Route
            path={ROUTES.ADMIN_SETTINGS}
            element={<AdminGlobalSettingsPage />}
          />
          <Route path={ROUTES.ADMIN_MAPPERS} element={<AdminMappersPage />} />
          <Route
            path={ROUTES.ADMIN_MAPPERS_PIPELINE}
            element={<AdminMappersPipelinePage />}
          />
        </Route>

        {/* Redirects */}
        <Route path="/hotkey-test" element={<HotkeyTest />} />
        <Route
          path={ROUTES.HOME}
          element={<Navigate to={ROUTES.DASHBOARD} replace />}
        />
        <Route path="*" element={<NotFoundPage />} />
      </Route>
    </Routes>
  )
}
