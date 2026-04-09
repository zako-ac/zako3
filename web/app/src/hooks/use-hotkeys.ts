import {
  useHotkeys as useHotkeysOriginal,
  Options,
  HotkeyCallback,
} from 'react-hotkeys-hook'
import { KeymapDefinition, ALL_KEYMAPS } from '@/lib/keymaps'

/**
 * Custom wrapper around react-hotkeys-hook
 * Integrates with centralized keymap registry
 */
export function useHotkeys(
  keys: string | string[],
  callback: HotkeyCallback,
  options?: Options,
  deps?: unknown[]
) {
  return useHotkeysOriginal(keys, callback, options, deps)
}

/**
 * Use hotkeys from centralized keymap registry
 * @example useKeymapHotkey('TOGGLE_SIDEBAR', () => toggleSidebar())
 */
export function useKeymapHotkey(
  keymapName: keyof typeof ALL_KEYMAPS,
  callback: HotkeyCallback,
  additionalOptions?: Omit<
    Options,
    'scopes' | 'preventDefault' | 'useKey' | 'ignoreModifiers'
  >,
  deps?: unknown[]
) {
  const keymap: KeymapDefinition | undefined = ALL_KEYMAPS[keymapName]

  // Convert scope to scopes array
  const scopes = keymap?.scope
    ? Array.isArray(keymap.scope)
      ? keymap.scope
      : [keymap.scope]
    : undefined

  const options: Options = {
    ...additionalOptions,
    preventDefault: keymap?.preventDefault,
    useKey: keymap?.useKey,
    ignoreModifiers: keymap?.ignoreModifiers,
    scopes,
    // If keymap not found, disable. Otherwise use additionalOptions.enabled (defaults to true if undefined)
    enabled: keymap ? additionalOptions?.enabled !== false : false,
  }

  // Always call the hook, but disable it if keymap not found
  const result = useHotkeys(keymap?.keys || '', callback, options, deps)

  // Warn in development if keymap not found
  if (!keymap && process.env.NODE_ENV === 'development') {
    console.warn(`Keymap "${String(keymapName)}" not found in registry`)
  }

  return result
}

// Re-export other hooks from react-hotkeys-hook
export { useHotkeysContext } from 'react-hotkeys-hook'
