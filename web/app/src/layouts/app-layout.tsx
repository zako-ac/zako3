import { Outlet } from 'react-router-dom'
import { SidebarProvider, SidebarInset } from '@/components/ui/sidebar'
import { AppSidebar } from '@/components/layout/app-sidebar'
import { Header, type BreadcrumbItem } from '@/components/layout/header'
import { useAuthStore } from '@/features/auth'

interface AppLayoutProps {
  breadcrumbs?: BreadcrumbItem[]
}

export const AppLayout = ({ breadcrumbs }: AppLayoutProps) => {
  const { isAuthenticated } = useAuthStore()

  return (
    <SidebarProvider>
      {isAuthenticated && <AppSidebar />}
      <SidebarInset>
        <Header breadcrumbs={breadcrumbs} />
        <main className="flex-1 overflow-auto p-4 md:p-6">
          <Outlet />
        </main>
      </SidebarInset>
    </SidebarProvider>
  )
}
