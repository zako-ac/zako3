import { ThemeProvider as NextThemesProvider } from 'next-themes'
import type { ReactNode } from 'react'
import { THEME_STORAGE_KEY } from '@/lib/constants'

interface ThemeProviderProps {
  children: ReactNode
  defaultTheme?: 'light' | 'dark' | 'system'
}

export const ThemeProvider = ({
  children,
  defaultTheme = 'dark',
}: ThemeProviderProps) => {
  return (
    <NextThemesProvider
      attribute="class"
      defaultTheme={defaultTheme}
      enableSystem
      storageKey={THEME_STORAGE_KEY}
    >
      {children}
    </NextThemesProvider>
  )
}
