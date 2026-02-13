/**
 * Platform detection utilities for keyboard shortcuts
 */

/**
 * Detect if the current platform is macOS
 */
export function isMac(): boolean {
  if (typeof window === 'undefined') return false
  return /Mac|iPhone|iPad|iPod/.test(navigator.platform)
}

/**
 * Get the modifier key symbol based on platform
 * @returns '⌘' for Mac, 'Ctrl' for others
 */
export function getModifierSymbol(): string {
  return isMac() ? '⌘' : 'Ctrl'
}

/**
 * Get the modifier key text based on platform
 * @returns 'Cmd' for Mac, 'Ctrl' for others
 */
export function getModifierText(): string {
  return isMac() ? 'Cmd' : 'Ctrl'
}

/**
 * Get the Alt key symbol based on platform
 * @returns '⌥' for Mac, 'Alt' for others
 */
export function getAltSymbol(): string {
  return isMac() ? '⌥' : 'Alt'
}

/**
 * Convert key string to display format
 * Handles platform-specific conversions
 */
export function formatKeyForDisplay(key: string): string {
  const keyMap: Record<string, string> = {
    mod: getModifierSymbol(),
    ctrl: 'Ctrl',
    cmd: '⌘',
    alt: getAltSymbol(),
    shift: '⇧',
    enter: '↵',
    escape: 'Esc',
    backspace: '⌫',
    delete: '⌦',
    tab: '⇥',
    up: '↑',
    down: '↓',
    left: '←',
    right: '→',
    space: 'Space',
    '?': '?',
    '/': '/',
  }

  return keyMap[key.toLowerCase()] || key.toUpperCase()
}

/**
 * Parse key combination string into individual keys
 * @example parseKeys('mod+shift+k') => ['⌘', '⇧', 'K'] (on Mac)
 */
export function parseKeys(keys: string): string[] {
  return keys.split('+').map((key) => formatKeyForDisplay(key.trim()))
}

/**
 * Parse key sequence (> for sequential, space for alternative)
 * @example parseKeySequence('g>d') => [['G'], ['D']]
 */
export function parseKeySequence(keys: string): string[][] {
  // Check if it's a sequence (>) or space-separated
  const separator = keys.includes('>') ? '>' : ' '
  return keys.split(separator).map((combo) => parseKeys(combo.trim()))
}
