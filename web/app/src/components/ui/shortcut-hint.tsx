import { Keyboard } from 'lucide-react'
import { KbdKeys, KbdSequence } from './kbd'
import { cn } from '@/lib/utils'

interface ShortcutHintProps {
  keys: string
  className?: string
  showIcon?: boolean
}

/**
 * Display keyboard shortcut hint with optional keyboard icon
 * Used inline with buttons, inputs, and menu items
 */
export function ShortcutHint({
  keys,
  className,
  showIcon = true,
}: ShortcutHintProps) {
  // Check if it's a sequence (contains > or space)
  const isSequence = keys.includes('>') || keys.includes(' ')

  return (
    <span
      className={cn(
        'text-muted-foreground ml-auto inline-flex items-center gap-1.5',
        className
      )}
    >
      {showIcon && <Keyboard className="size-3 opacity-60" />}
      {isSequence ? <KbdSequence keys={keys} /> : <KbdKeys keys={keys} />}
    </span>
  )
}
