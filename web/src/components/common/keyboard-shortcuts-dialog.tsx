import { useState } from 'react'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { KbdKeys, KbdSequence } from '@/components/ui/kbd'
import { KEYMAP_CATEGORIES } from '@/lib/keymaps'
import { useKeymapHotkey } from '@/hooks/use-hotkeys'
import { Keyboard } from 'lucide-react'

export function KeyboardShortcutsDialog() {
  const [isOpen, setIsOpen] = useState(false)

  // Listen for ? key to open dialog
  useKeymapHotkey('SHOW_HELP', () => setIsOpen(true))

  // Close on Escape
  useKeymapHotkey('CLOSE_DIALOG', () => setIsOpen(false))

  return (
    <Dialog open={isOpen} onOpenChange={setIsOpen}>
      <DialogContent className="max-h-[80vh] max-w-2xl overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Keyboard className="size-5" />
            Keyboard Shortcuts
          </DialogTitle>
          <DialogDescription>
            Available keyboard shortcuts for quick navigation and actions
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-6">
          {KEYMAP_CATEGORIES.map((category) => (
            <div key={category.name}>
              <h3 className="mb-3 text-sm font-semibold">{category.name}</h3>
              <p className="text-muted-foreground mb-3 text-xs">
                {category.description}
              </p>
              <div className="space-y-2">
                {Object.entries(category.shortcuts).map(([key, shortcut]) => (
                  <div
                    key={key}
                    className="hover:bg-muted/50 flex items-center justify-between rounded-md px-3 py-2 transition-colors"
                  >
                    <span className="text-sm">{shortcut.description}</span>
                    {shortcut.keys.includes('>') ||
                    shortcut.keys.includes(' ') ? (
                      <KbdSequence keys={shortcut.keys} />
                    ) : (
                      <KbdKeys keys={shortcut.keys} />
                    )}
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>

        <div className="mt-4 border-t pt-4">
          <p className="text-muted-foreground text-center text-xs">
            Press <KbdKeys keys="escape" className="mx-1 inline-flex" /> to
            close this dialog
          </p>
        </div>
      </DialogContent>
    </Dialog>
  )
}
