/**
 * KEYBOARD SHORTCUTS IMPLEMENTATION EXAMPLES
 *
 * This file demonstrates how to use the new keyboard shortcuts system.
 * All keyboard shortcuts are centralized in src/lib/keymaps.ts
 */

import { Button } from '@/components/ui/button'
import { ShortcutHint } from '@/components/ui/shortcut-hint'
import {
  NavigableTabs,
  TabsList,
  TabsTrigger,
  TabsContent,
} from '@/components/ui/tabs-navigable'
import { KbdKeys, KbdSequence } from '@/components/ui/kbd'
import { useKeymapHotkey } from '@/hooks/use-hotkeys'

// ============================================================================
// EXAMPLE 1: Using ShortcutHint with Buttons
// ============================================================================

export function ButtonWithShortcut() {
  return (
    <Button>
      Save
      <ShortcutHint keys="mod+s" />
    </Button>
  )
}

// ============================================================================
// EXAMPLE 2: Displaying Keyboard Shortcuts
// ============================================================================

export function DisplayShortcuts() {
  return (
    <div className="space-y-2">
      {/* Single key combination */}
      <div className="flex items-center justify-between">
        <span>Toggle sidebar</span>
        <KbdKeys keys="mod+b" />
      </div>

      {/* Key sequence (press one after another) */}
      <div className="flex items-center justify-between">
        <span>Go to dashboard</span>
        <KbdSequence keys="g>d" />
      </div>

      {/* With keyboard icon hint */}
      <div className="flex items-center justify-between">
        <span>Open search</span>
        <ShortcutHint keys="mod+k" showIcon={true} />
      </div>

      {/* Without keyboard icon */}
      <div className="flex items-center justify-between">
        <span>Show help</span>
        <ShortcutHint keys="?" showIcon={false} />
      </div>
    </div>
  )
}

// ============================================================================
// EXAMPLE 3: Using Keyboard-Navigable Tabs
// ============================================================================

export function TabsExample() {
  return (
    <NavigableTabs
      defaultValue="tab1"
      enableKeyboardNav={true}
      showKeyboardHint={true}
    >
      <TabsList>
        <TabsTrigger value="tab1">Tab 1</TabsTrigger>
        <TabsTrigger value="tab2">Tab 2</TabsTrigger>
        <TabsTrigger value="tab3">Tab 3</TabsTrigger>
      </TabsList>
      <TabsContent value="tab1">
        Content 1 - Press Alt+Down to go to next tab
      </TabsContent>
      <TabsContent value="tab2">
        Content 2 - Press Alt+Up to go to previous tab
      </TabsContent>
      <TabsContent value="tab3">Content 3</TabsContent>
    </NavigableTabs>
  )
}

// ============================================================================
// EXAMPLE 4: Using Centralized Keymaps in Components
// ============================================================================

export function ComponentWithHotkeys() {
  const handleSave = () => {
    console.log('Saving...')
  }

  // Use keymap from centralized registry
  useKeymapHotkey('SAVE', handleSave)

  return (
    <div>
      <p>Press Cmd/Ctrl+S to save</p>
      <Button onClick={handleSave}>
        Save
        <ShortcutHint keys="mod+s" />
      </Button>
    </div>
  )
}

// ============================================================================
// AVAILABLE GLOBAL SHORTCUTS
// ============================================================================

/**
 * These shortcuts are always available:
 *
 * - ?               : Show keyboard shortcuts help dialog
 * - Mod+B           : Toggle sidebar
 * - Mod+K           : Open command palette
 * - Escape          : Close dialog/modal
 * - G > D           : Go to dashboard
 * - G > T           : Go to taps
 * - G > S           : Go to settings
 *
 * When tabs are focused:
 * - Alt+Down          : Next tab
 * - Alt+Up            : Previous tab
 *
 * When editing:
 * - Mod+S             : Save
 * - Escape            : Cancel editing
 */

// ============================================================================
// ADDING NEW SHORTCUTS
// ============================================================================

/**
 * To add new shortcuts:
 *
 * 1. Add the shortcut definition to src/lib/keymaps.ts
 * 2. Use useKeymapHotkey('SHORTCUT_NAME', callback) in your component
 * 3. Add ShortcutHint to your UI elements
 *
 * Example:
 *
 * // In src/lib/keymaps.ts
 * export const MY_KEYMAPS = {
 *   DELETE: {
 *     keys: 'mod+shift+d',
 *     description: 'Delete item',
 *     preventDefault: true,
 *     scope: 'global',
 *   },
 * }
 *
 * // In your component
 * useKeymapHotkey('DELETE', handleDelete)
 *
 * // In your UI
 * <Button onClick={handleDelete}>
 *   Delete
 *   <ShortcutHint keys="mod+shift+d" />
 * </Button>
 */
