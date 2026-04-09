import { Outlet, useNavigate } from 'react-router-dom'
import { useKeymapHotkey } from '@/hooks/use-hotkeys'
import { KeyboardShortcutsDialog } from '@/components/common/keyboard-shortcuts-dialog'
import { ROUTES } from '@/lib/constants'

export const RootLayout = () => {
  const navigate = useNavigate()

  // Global navigation shortcuts
  useKeymapHotkey('GO_TO_DASHBOARD', () => navigate(ROUTES.DASHBOARD))
  useKeymapHotkey('GO_TO_TAPS', () => navigate(ROUTES.TAPS))
  useKeymapHotkey('GO_TO_SETTINGS', () => navigate(ROUTES.SETTINGS))

  return (
    <div className="bg-background text-foreground min-h-svh antialiased">
      <Outlet />
      <KeyboardShortcutsDialog />
    </div>
  )
}
