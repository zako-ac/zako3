'use client'

import * as React from 'react'
import * as TabsPrimitive from '@radix-ui/react-tabs'
import { cn } from '@/lib/utils'
import { useKeymapHotkey, useHotkeysContext } from '@/hooks/use-hotkeys'
import { ShortcutHint } from './shortcut-hint'

interface NavigableTabsProps extends React.ComponentProps<
  typeof TabsPrimitive.Root
> {
  enableKeyboardNav?: boolean
  showKeyboardHint?: boolean
}

/**
 * Enhanced Tabs component with keyboard navigation (Alt+Up/Down)
 * Wraps around the existing Tabs component
 */
function NavigableTabs({
  className,
  enableKeyboardNav = true,
  showKeyboardHint = true,
  children,
  value,
  defaultValue,
  onValueChange,
  ...props
}: NavigableTabsProps) {
  const [internalValue, setInternalValue] = React.useState(
    value || defaultValue || ''
  )
  const tabValuesRef = React.useRef<string[]>([])
  const { enableScope, disableScope } = useHotkeysContext()

  // Sync with external value changes
  React.useEffect(() => {
    if (value !== undefined) {
      setInternalValue(value)
    }
  }, [value])

  const currentValue = value !== undefined ? value : internalValue

  const handleValueChange = React.useCallback(
    (newValue: string) => {
      if (value === undefined) {
        setInternalValue(newValue)
      }
      onValueChange?.(newValue)
    },
    [value, onValueChange]
  )

  // Navigate to next tab (Alt+Down)
  useKeymapHotkey(
    'NEXT_TAB',
    () => {
      if (!enableKeyboardNav || tabValuesRef.current.length === 0) return

      const currentIndex = tabValuesRef.current.indexOf(currentValue)
      const nextIndex = (currentIndex + 1) % tabValuesRef.current.length
      const nextValue = tabValuesRef.current[nextIndex]
      handleValueChange(nextValue)
    },
    { enabled: enableKeyboardNav },
    [currentValue, enableKeyboardNav, handleValueChange]
  )

  // Navigate to previous tab (Alt+Up)
  useKeymapHotkey(
    'PREV_TAB',
    () => {
      if (!enableKeyboardNav || tabValuesRef.current.length === 0) return

      const currentIndex = tabValuesRef.current.indexOf(currentValue)
      const prevIndex =
        (currentIndex - 1 + tabValuesRef.current.length) %
        tabValuesRef.current.length
      const prevValue = tabValuesRef.current[prevIndex]
      handleValueChange(prevValue)
    },
    { enabled: enableKeyboardNav },
    [currentValue, enableKeyboardNav, handleValueChange]
  )

  // Enable tabs scope when component mounts
  React.useEffect(() => {
    if (enableKeyboardNav) {
      enableScope('tabs')
      return () => disableScope('tabs')
    }
  }, [enableKeyboardNav, enableScope, disableScope])

  // Extract tab values from children
  React.useEffect(() => {
    const values: string[] = []

    const extractValues = (children: React.ReactNode) => {
      React.Children.forEach(children, (child) => {
        if (React.isValidElement(child)) {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          const childProps = child.props as any
          if (
            child.type === TabsPrimitive.Trigger ||
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            (child.type as any).displayName === 'TabsTrigger'
          ) {
            const value = childProps.value
            if (value) values.push(value)
          }
          if (childProps.children) {
            extractValues(childProps.children)
          }
        }
      })
    }

    extractValues(children)
    tabValuesRef.current = values
  }, [children])

  return (
    <TabsPrimitive.Root
      data-slot="tabs"
      className={cn('flex flex-col gap-2', className)}
      value={currentValue}
      onValueChange={handleValueChange}
      {...props}
    >
      {children}
      {showKeyboardHint && enableKeyboardNav && (
        <div className="text-muted-foreground mt-1 flex items-center gap-2 text-xs">
          <ShortcutHint keys="alt+up" showIcon={false} />
          <span>/</span>
          <ShortcutHint keys="alt+down" showIcon={false} />
          <span>to navigate</span>
        </div>
      )}
    </TabsPrimitive.Root>
  )
}

// Re-export existing tab components
export { NavigableTabs }
export { TabsList, TabsTrigger, TabsContent } from './tabs'
