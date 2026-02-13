import { AppProviders } from '@/app/providers'
import { AppRouter } from '@/app/router'

export const App = () => {
  return (
    <AppProviders>
      <AppRouter />
    </AppProviders>
  )
}
