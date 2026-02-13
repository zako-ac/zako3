import { Outlet } from 'react-router-dom'
import { SidebarProvider, SidebarInset } from '@/components/ui/sidebar'
import { AdminSidebar } from '@/components/layout/admin-sidebar'
import { Header, type BreadcrumbItem } from '@/components/layout/header'

interface AdminLayoutProps {
  breadcrumbs?: BreadcrumbItem[]
}

export const AdminLayout = ({ breadcrumbs }: AdminLayoutProps) => {
  return (
    <SidebarProvider>
      <AdminSidebar />
      <SidebarInset>
        <Header breadcrumbs={breadcrumbs} />
        <main className="flex-1 overflow-auto p-4 md:p-6">
          <Outlet />
        </main>
      </SidebarInset>
    </SidebarProvider>
  )
}
