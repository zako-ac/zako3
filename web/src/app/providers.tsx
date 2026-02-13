import { QueryClientProvider } from '@tanstack/react-query'
import { ReactQueryDevtools } from '@tanstack/react-query-devtools'
import type { ReactNode } from 'react'
import { BrowserRouter } from 'react-router-dom'
import { HotkeysProvider } from 'react-hotkeys-hook'
import { queryClient } from './query-client'
import { ThemeProvider } from '@/components/layout/theme-provider'
import { Toaster } from '@/components/ui/sonner'
import { I18nextProvider } from 'react-i18next'
import i18n from '@/i18n/config'

interface AppProvidersProps {
  children: ReactNode
}

export const AppProviders = ({ children }: AppProvidersProps) => {
  return (
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={queryClient}>
        <ThemeProvider defaultTheme="dark">
          <HotkeysProvider initiallyActiveScopes={['global']}>
            <BrowserRouter>
              {children}
              <Toaster position="top-right" richColors closeButton />
            </BrowserRouter>
          </HotkeysProvider>
        </ThemeProvider>
        <ReactQueryDevtools initialIsOpen={false} />
      </QueryClientProvider>
    </I18nextProvider>
  )
}
