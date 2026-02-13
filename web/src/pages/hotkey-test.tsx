import { useEffect } from 'react'
import { useKeymapHotkey } from '@/hooks/use-hotkeys'
import { Button } from '@/components/ui/button'
import { ShortcutHint } from '@/components/ui/shortcut-hint'
import {
  NavigableTabs,
  TabsList,
  TabsTrigger,
  TabsContent,
} from '@/components/ui/tabs-navigable'

export function HotkeyTest() {
  useEffect(() => {
    console.log('HotkeyTest component mounted')
  }, [])

  useKeymapHotkey('TOGGLE_SIDEBAR', () => {
    console.log('✅ TOGGLE_SIDEBAR (mod+b) works!')
    alert('TOGGLE_SIDEBAR works!')
  })

  useKeymapHotkey('SHOW_HELP', () => {
    console.log('✅ SHOW_HELP (?) works!')
    alert('SHOW_HELP works!')
  })

  useKeymapHotkey('GO_TO_DASHBOARD', () => {
    console.log('✅ GO_TO_DASHBOARD (g>d) works!')
    alert('GO_TO_DASHBOARD works!')
  })

  useKeymapHotkey('OPEN_SEARCH', () => {
    console.log('✅ OPEN_SEARCH (mod+k) works!')
    alert('OPEN_SEARCH works! Browser search should NOT open.')
  })

  const testClick = () => {
    console.log('Button clicked!')
    alert('Button works!')
  }

  return (
    <div className="flex min-h-screen items-center justify-center p-8">
      <div className="w-full max-w-2xl space-y-8">
        <div className="space-y-4">
          <h1 className="text-2xl font-bold">Hotkey Test Page</h1>
          <p className="text-muted-foreground">Test keyboard shortcuts:</p>
          <ul className="space-y-2 text-sm">
            <li className="flex items-center justify-between">
              <span>Toggle sidebar</span>
              <ShortcutHint keys="mod+b" />
            </li>
            <li className="flex items-center justify-between">
              <span>Show help</span>
              <ShortcutHint keys="?" showIcon={false} />
            </li>
            <li className="flex items-center justify-between">
              <span>Open search</span>
              <ShortcutHint keys="mod+k" />
            </li>
            <li className="flex items-center justify-between">
              <span>Go to dashboard</span>
              <ShortcutHint keys="g>d" />
            </li>
          </ul>
          <Button onClick={testClick} className="w-full">
            Test Button Click
          </Button>
        </div>

        <div className="space-y-4">
          <h2 className="text-xl font-bold">Tab Navigation Test</h2>
          <p className="text-muted-foreground text-sm">
            Use Alt+Up/Down to navigate between tabs
          </p>
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
            <TabsContent value="tab1" className="rounded-md border p-4">
              <h3 className="font-semibold">Tab 1 Content</h3>
              <p className="text-muted-foreground text-sm">
                Press Alt+Down to go to Tab 2
              </p>
            </TabsContent>
            <TabsContent value="tab2" className="rounded-md border p-4">
              <h3 className="font-semibold">Tab 2 Content</h3>
              <p className="text-muted-foreground text-sm">
                Press Alt+Up to go to Tab 1, or Alt+Down to go to Tab 3
              </p>
            </TabsContent>
            <TabsContent value="tab3" className="rounded-md border p-4">
              <h3 className="font-semibold">Tab 3 Content</h3>
              <p className="text-muted-foreground text-sm">
                Press Alt+Up to go to Tab 2
              </p>
            </TabsContent>
          </NavigableTabs>
        </div>

        <p className="text-muted-foreground text-center text-xs">
          Open browser console to see logs
        </p>
      </div>
    </div>
  )
}
