/**
 * Centralized keyboard shortcuts registry
 * All keyboard shortcuts are defined here for easy configuration
 */

export interface KeymapDefinition {
    keys: string
    description: string
    scope?: string | string[]
    preventDefault?: boolean
    useKey?: boolean
    ignoreModifiers?: boolean
}

export type KeymapCategory = {
    name: string
    description: string
    shortcuts: Record<string, KeymapDefinition>
}

/**
 * Global keyboard shortcuts - always active
 */
export const GLOBAL_KEYMAPS: Record<string, KeymapDefinition> = {
    TOGGLE_SIDEBAR: {
        keys: 'mod+b',
        description: 'Toggle sidebar',
        preventDefault: true,
        scope: 'global',
    },
    SHOW_HELP: {
        keys: '?',
        description: 'Show keyboard shortcuts',
        preventDefault: true,
        scope: 'global',
        useKey: true,
    },
    OPEN_SEARCH: {
        keys: 'mod+k',
        description: 'Open command palette',
        preventDefault: true,
        scope: 'global',
    },
    CLOSE_DIALOG: {
        keys: 'escape',
        description: 'Close dialog or modal',
        preventDefault: false,
        scope: 'global',
    },
}

/**
 * Navigation keyboard shortcuts
 */
export const NAVIGATION_KEYMAPS: Record<string, KeymapDefinition> = {
    NEXT_TAB: {
        keys: 'alt+up',
        description: 'Navigate to next tab',
        preventDefault: true,
        scope: 'tabs',
    },
    PREV_TAB: {
        keys: 'alt+down',
        description: 'Navigate to previous tab',
        preventDefault: true,
        scope: 'tabs',
    },
    GO_TO_DASHBOARD: {
        keys: 'g>d',
        description: 'Go to dashboard',
        preventDefault: true,
        scope: 'global',
    },
    GO_TO_TAPS: {
        keys: 'g>t',
        description: 'Go to taps',
        preventDefault: true,
        scope: 'global',
    },
    GO_TO_SETTINGS: {
        keys: 'g>s',
        description: 'Go to settings',
        preventDefault: true,
        scope: 'global',
    },
}

/**
 * Editor keyboard shortcuts - active in text editing contexts
 */
export const EDITOR_KEYMAPS: Record<string, KeymapDefinition> = {
    SAVE: {
        keys: 'mod+s',
        description: 'Save',
        preventDefault: true,
        scope: 'editor',
    },
    CANCEL: {
        keys: 'escape',
        description: 'Cancel editing',
        preventDefault: false,
        scope: 'editor',
    },
}

/**
 * All keymap categories for display in help dialog
 */
export const KEYMAP_CATEGORIES: KeymapCategory[] = [
    {
        name: 'Global',
        description: 'Available everywhere',
        shortcuts: GLOBAL_KEYMAPS,
    },
    {
        name: 'Navigation',
        description: 'Navigate around the app',
        shortcuts: NAVIGATION_KEYMAPS,
    },
    {
        name: 'Editing',
        description: 'Active when editing',
        shortcuts: EDITOR_KEYMAPS,
    },
]

/**
 * Get all keymaps as a flat object
 */
export const ALL_KEYMAPS = {
    ...GLOBAL_KEYMAPS,
    ...NAVIGATION_KEYMAPS,
    ...EDITOR_KEYMAPS,
}

/**
 * Helper to get keymap by name
 */
export function getKeymap(name: keyof typeof ALL_KEYMAPS): KeymapDefinition {
    return ALL_KEYMAPS[name]
}
