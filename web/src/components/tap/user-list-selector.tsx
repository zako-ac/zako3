import { useState, useRef } from 'react'
import { X, Upload, Download, UserCircle, HelpCircle } from 'lucide-react'
import { useUsers } from '@/features/users'
import { Button } from '@/components/ui/button'
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from '@/components/ui/command'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { Badge } from '@/components/ui/badge'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import { toast } from 'sonner'
import { cn } from '@/lib/utils'

interface SelectedUser {
  id: string
  username?: string
  avatar?: string
  exists: boolean
}

interface UserListSelectorProps {
  value: string[]
  onChange: (userIds: string[]) => void
  label?: string
  placeholder?: string
  description?: string
  disabled?: boolean
}

export const UserListSelector = ({
  value = [],
  onChange,
  label = 'Users',
  placeholder = 'Add users...',
  description,
  disabled = false,
}: UserListSelectorProps) => {
  const [open, setOpen] = useState(false)
  const [search, setSearch] = useState('')
  const fileInputRef = useRef<HTMLInputElement>(null)

  // Fetch users for search
  const { data: usersData } = useUsers({ search, perPage: 50 })
  const users = usersData?.data || []

  // Build selected users list with metadata
  const selectedUsers: SelectedUser[] = value.map((id) => {
    const user = users.find((u) => u.id === id)
    return {
      id,
      username: user?.username,
      avatar: user?.avatar,
      exists: !!user,
    }
  })

  const handleSelect = (userId: string) => {
    if (value.includes(userId)) {
      onChange(value.filter((id) => id !== userId))
    } else {
      onChange([...value, userId])
    }
  }

  const handleRemove = (userId: string) => {
    onChange(value.filter((id) => id !== userId))
  }

  const handleClearAll = () => {
    onChange([])
  }

  const handleExportJSON = () => {
    // eslint-disable-next-line react-hooks/purity
    const timestamp = Date.now()
    const data = JSON.stringify(value, null, 2)
    const blob = new Blob([data], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `user-list-${timestamp}.json`
    a.click()
    URL.revokeObjectURL(url)
    toast.success('User list exported as JSON')
  }

  const handleExportCSV = () => {
    // eslint-disable-next-line react-hooks/purity
    const timestamp = Date.now()
    const data = value.join(',')
    const blob = new Blob([data], { type: 'text/csv' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `user-list-${timestamp}.csv`
    a.click()
    URL.revokeObjectURL(url)
    toast.success('User list exported as CSV')
  }

  const handleImport = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]
    if (!file) return

    const reader = new FileReader()
    reader.onload = (e) => {
      try {
        const content = e.target?.result as string
        let userIds: string[] = []

        // Try parsing as JSON first
        if (file.name.endsWith('.json')) {
          const parsed = JSON.parse(content)
          userIds = Array.isArray(parsed) ? parsed : []
        } else {
          // Parse as CSV (comma or newline separated)
          userIds = content
            .split(/[,\n]/)
            .map((id) => id.trim())
            .filter((id) => id.length > 0)
        }

        // Merge with existing, removing duplicates
        const merged = [...new Set([...value, ...userIds])]
        onChange(merged)
        toast.success(`Imported ${userIds.length} user(s)`)
      } catch {
        toast.error('Failed to parse file. Please check the format.')
      }
    }
    reader.readAsText(file)

    // Reset file input
    if (fileInputRef.current) {
      fileInputRef.current.value = ''
    }
  }

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <label className="text-sm font-medium">{label}</label>
          {description && (
            <TooltipProvider>
              <Tooltip>
                <TooltipTrigger asChild>
                  <HelpCircle className="text-muted-foreground h-4 w-4 cursor-help" />
                </TooltipTrigger>
                <TooltipContent>
                  <p className="max-w-xs">{description}</p>
                </TooltipContent>
              </Tooltip>
            </TooltipProvider>
          )}
        </div>
        <div className="flex items-center gap-2">
          <input
            ref={fileInputRef}
            type="file"
            accept=".json,.csv"
            onChange={handleImport}
            className="hidden"
            disabled={disabled}
          />
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => fileInputRef.current?.click()}
            disabled={disabled}
          >
            <Upload className="mr-2 h-4 w-4" />
            Import
          </Button>
          <Popover>
            <PopoverTrigger asChild>
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={value.length === 0 || disabled}
              >
                <Download className="mr-2 h-4 w-4" />
                Export
              </Button>
            </PopoverTrigger>
            <PopoverContent className="w-40 p-2" align="end">
              <div className="flex flex-col gap-1">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={handleExportJSON}
                  className="justify-start"
                >
                  Export as JSON
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={handleExportCSV}
                  className="justify-start"
                >
                  Export as CSV
                </Button>
              </div>
            </PopoverContent>
          </Popover>
        </div>
      </div>

      {/* Selected users */}
      {selectedUsers.length > 0 && (
        <div className="bg-muted/30 flex flex-wrap gap-2 rounded-md border p-3">
          {selectedUsers.map((user) => (
            <TooltipProvider key={user.id}>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Badge
                    variant={user.exists ? 'secondary' : 'outline'}
                    className={cn(
                      'flex items-center gap-2 pr-1',
                      !user.exists && 'border-warning text-warning'
                    )}
                  >
                    <Avatar className="h-5 w-5">
                      {user.avatar ? (
                        <AvatarImage
                          src={user.avatar}
                          alt={user.username || user.id}
                        />
                      ) : (
                        <AvatarFallback>
                          <UserCircle className="h-4 w-4" />
                        </AvatarFallback>
                      )}
                    </Avatar>
                    <span className="max-w-[120px] truncate">
                      {user.username || user.id}
                    </span>
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      className="h-4 w-4 p-0 hover:bg-transparent"
                      onClick={() => handleRemove(user.id)}
                      disabled={disabled}
                    >
                      <X className="h-3 w-3" />
                    </Button>
                  </Badge>
                </TooltipTrigger>
                {!user.exists && (
                  <TooltipContent>
                    <p className="max-w-xs">
                      This user doesn't exist yet. This is allowed - they may be
                      added later.
                    </p>
                  </TooltipContent>
                )}
              </Tooltip>
            </TooltipProvider>
          ))}
          {selectedUsers.length > 0 && (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={handleClearAll}
              disabled={disabled}
              className="h-6 px-2 text-xs"
            >
              Clear all
            </Button>
          )}
        </div>
      )}

      {/* User search/add */}
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <Button
            type="button"
            variant="outline"
            className="w-full justify-start text-left font-normal"
            disabled={disabled}
          >
            <UserCircle className="mr-2 h-4 w-4" />
            {placeholder}
          </Button>
        </PopoverTrigger>
        <PopoverContent className="w-[400px] p-0" align="start">
          <Command shouldFilter={false}>
            <CommandInput
              placeholder="Search users by ID or username..."
              value={search}
              onValueChange={setSearch}
            />
            <CommandList>
              <CommandEmpty>
                {search ? (
                  <div className="p-4 text-sm">
                    <p className="mb-2">No users found.</p>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => {
                        if (!value.includes(search)) {
                          handleSelect(search)
                          setSearch('')
                          toast.info(
                            'Added user ID. Note: This user may not exist yet.'
                          )
                        }
                      }}
                      className="w-full"
                    >
                      Add "{search}" as user ID
                    </Button>
                  </div>
                ) : (
                  'Start typing to search users...'
                )}
              </CommandEmpty>
              <CommandGroup>
                {users.map((user) => (
                  <CommandItem
                    key={user.id}
                    value={user.id}
                    onSelect={() => handleSelect(user.id)}
                    className="flex cursor-pointer items-center gap-2"
                  >
                    <div
                      className={cn(
                        'mr-2 flex h-4 w-4 items-center justify-center rounded-sm border',
                        value.includes(user.id) && 'bg-primary border-primary'
                      )}
                    >
                      {value.includes(user.id) && (
                        <div className="h-2 w-2 rounded-sm bg-white" />
                      )}
                    </div>
                    <Avatar className="h-6 w-6">
                      <AvatarImage src={user.avatar} alt={user.username} />
                      <AvatarFallback>
                        {user.username.slice(0, 2).toUpperCase()}
                      </AvatarFallback>
                    </Avatar>
                    <div className="flex min-w-0 flex-1 flex-col">
                      <span className="truncate text-sm font-medium">
                        {user.username}
                      </span>
                      <span className="text-muted-foreground truncate text-xs">
                        {user.id}
                      </span>
                    </div>
                  </CommandItem>
                ))}
              </CommandGroup>
            </CommandList>
          </Command>
        </PopoverContent>
      </Popover>
    </div>
  )
}
