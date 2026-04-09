import { cn } from '@/lib/utils'
import { parseKeys, parseKeySequence } from '@/lib/platform'

function Kbd({ className, ...props }: React.ComponentProps<'kbd'>) {
  return (
    <kbd
      data-slot="kbd"
      className={cn(
        'bg-muted text-muted-foreground pointer-events-none inline-flex h-5 w-fit min-w-5 items-center justify-center gap-1 rounded-sm px-1 font-sans text-xs font-medium select-none',
        "[&_svg:not([class*='size-'])]:size-3",
        '[[data-slot=tooltip-content]_&]:bg-background/20 [[data-slot=tooltip-content]_&]:text-background dark:[[data-slot=tooltip-content]_&]:bg-background/10',
        className
      )}
      {...props}
    />
  )
}

function KbdGroup({ className, ...props }: React.ComponentProps<'div'>) {
  return (
    <kbd
      data-slot="kbd-group"
      className={cn('inline-flex items-center gap-1', className)}
      {...props}
    />
  )
}

/**
 * Render keyboard shortcut from key string
 * @example <KbdKeys keys="mod+k" /> => <Kbd>⌘</Kbd><Kbd>K</Kbd> (on Mac)
 */
function KbdKeys({ keys, className }: { keys: string; className?: string }) {
  const parsedKeys = parseKeys(keys)

  return (
    <KbdGroup className={className}>
      {parsedKeys.map((key, index) => (
        <Kbd key={index}>{key}</Kbd>
      ))}
    </KbdGroup>
  )
}

/**
 * Render keyboard sequence (space-separated combinations)
 * @example <KbdSequence keys="g d" /> => <Kbd>G</Kbd> then <Kbd>D</Kbd>
 */
function KbdSequence({
  keys,
  className,
}: {
  keys: string
  className?: string
}) {
  const sequence = parseKeySequence(keys)

  return (
    <div className={cn('inline-flex items-center gap-1', className)}>
      {sequence.map((combo, seqIndex) => (
        <span key={seqIndex} className="inline-flex items-center gap-1">
          {seqIndex > 0 && (
            <span className="text-muted-foreground text-xs">then</span>
          )}
          <KbdGroup>
            {combo.map((key, keyIndex) => (
              <Kbd key={keyIndex}>{key}</Kbd>
            ))}
          </KbdGroup>
        </span>
      ))}
    </div>
  )
}

export { Kbd, KbdGroup, KbdKeys, KbdSequence }
