import { useState, useCallback } from 'react'

interface UseClipboardOptions {
  timeout?: number
}

interface UseClipboardReturn {
  copied: boolean
  copy: (text: string) => Promise<void>
}

export const useClipboard = (
  options: UseClipboardOptions = {}
): UseClipboardReturn => {
  const { timeout = 2000 } = options
  const [copied, setCopied] = useState(false)

  const copy = useCallback(
    async (text: string) => {
      try {
        await navigator.clipboard.writeText(text)
        setCopied(true)
        setTimeout(() => setCopied(false), timeout)
      } catch (error) {
        console.error('Failed to copy to clipboard:', error)
      }
    },
    [timeout]
  )

  return { copied, copy }
}
