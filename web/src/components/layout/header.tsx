import { SidebarTrigger } from '@/components/ui/sidebar'
import { Separator } from '@/components/ui/separator'
import { ThemeToggle } from './theme-toggle'
import { LanguageToggle } from './language-toggle'
import { NotificationBell } from '@/components/dashboard/notification-bell'
import { useAuthStore } from '@/features/auth'
import { Link } from 'react-router-dom'
import { ROUTES } from '@/lib/constants'
import zakoLogo from '@/assets/zakopsa.png'
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from '@/components/ui/breadcrumb'

export interface BreadcrumbItem {
  label: string
  href?: string
}

interface HeaderProps {
  breadcrumbs?: BreadcrumbItem[]
}

export const Header = ({ breadcrumbs }: HeaderProps) => {
  const { isAuthenticated } = useAuthStore()

  return (
    <header className="flex h-16 shrink-0 items-center justify-between gap-2 border-b px-4">
      <div className="flex items-center gap-2">
        {!isAuthenticated ? (
          <Link to={ROUTES.HOME} className="flex items-center gap-2 mr-2">
            <img className="h-6 w-6 rounded-lg" alt="ZAKO" src={zakoLogo} />
            <span className="text-lg font-semibold">ZAKO</span>
          </Link>
        ) : (
          <>
            <SidebarTrigger className="-ml-1" />
            <Separator orientation="vertical" className="mr-2 h-4" />
          </>
        )}
        {breadcrumbs && breadcrumbs.length > 0 && (
          <Breadcrumb>
            <BreadcrumbList>
              {breadcrumbs.map((item, index) => (
                <BreadcrumbItem key={index}>
                  {index > 0 && <BreadcrumbSeparator />}
                  {item.href ? (
                    <BreadcrumbLink href={item.href}>{item.label}</BreadcrumbLink>
                  ) : (
                    <BreadcrumbPage>{item.label}</BreadcrumbPage>
                  )}
                </BreadcrumbItem>
              ))}
            </BreadcrumbList>
          </Breadcrumb>
        )}
      </div>
      <div className="flex items-center gap-1">
        {isAuthenticated && <NotificationBell />}
        <LanguageToggle />
        <ThemeToggle />
      </div>
    </header>
  )
}
