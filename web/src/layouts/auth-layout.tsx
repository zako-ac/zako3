import { Outlet } from 'react-router-dom'
import { ThemeToggle } from '@/components/layout/theme-toggle'
import { LanguageToggle } from '@/components/layout/language-toggle'

export const AuthLayout = () => {
  return (
    <div className="flex min-h-svh flex-col">
      <header className="flex items-center justify-end gap-1 p-4">
        <LanguageToggle />
        <ThemeToggle />
      </header>
      <main className="flex flex-1 items-center justify-center p-4">
        <Outlet />
      </main>
      <footer className="p-4 text-center text-sm text-muted-foreground">
        &copy; {new Date().getFullYear()} ZAKO. All rights reserved.
      </footer>
    </div>
  )
}
