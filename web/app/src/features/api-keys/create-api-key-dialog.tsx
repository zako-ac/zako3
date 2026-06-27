import { useState } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { useTranslation } from 'react-i18next'
import { Copy, Check, AlertTriangle } from 'lucide-react'
import { toast } from 'sonner'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog'
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from '@/components/ui/form'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { useClipboard } from '@/hooks/use-clipboard'
import { createUserApiKeySchema, type CreateUserApiKeyInput } from '@zako-ac/zako3-data'

interface CreateApiKeyDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onSubmit: (data: CreateUserApiKeyInput) => Promise<{ token: string }>
  isLoading?: boolean
}

export const CreateApiKeyDialog = ({
  open,
  onOpenChange,
  onSubmit,
  isLoading,
}: CreateApiKeyDialogProps) => {
  const { t } = useTranslation()
  const { copied, copy } = useClipboard()
  const [newToken, setNewToken] = useState<string | null>(null)

  const form = useForm<CreateUserApiKeyInput>({
    resolver: zodResolver(createUserApiKeySchema),
    defaultValues: {
      label: '',
      expiry: 'never',
    },
  })

  const handleSubmit = async (data: CreateUserApiKeyInput) => {
    try {
      const result = await onSubmit(data)
      setNewToken(result.token)
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Failed to create key')
    }
  }

  const handleCopy = async () => {
    if (newToken) {
      await copy(newToken)
      toast.success(t('settings.apiKeys.copied'))
    }
  }

  const handleOpenChange = (newOpen: boolean) => {
    onOpenChange(newOpen)
    if (!newOpen) {
      setTimeout(() => {
        form.reset()
        setNewToken(null)
      }, 300)
    }
  }

  const handleClose = () => handleOpenChange(false)

  const expiryOptions = [
    { value: '1_month', label: t('settings.apiKeys.expiry.1_month') },
    { value: '3_months', label: t('settings.apiKeys.expiry.3_months') },
    { value: '6_months', label: t('settings.apiKeys.expiry.6_months') },
    { value: '1_year', label: t('settings.apiKeys.expiry.1_year') },
    { value: 'never', label: t('settings.apiKeys.expiry.never') },
  ] as const

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="sm:max-w-[500px]">
        <DialogHeader>
          <DialogTitle>{t('settings.apiKeys.createKey')}</DialogTitle>
          <DialogDescription>
            {newToken
              ? t('settings.apiKeys.createKeySuccess')
              : t('settings.apiKeys.createKeyDescription')}
          </DialogDescription>
        </DialogHeader>

        {!newToken ? (
          <Form {...form}>
            <form onSubmit={form.handleSubmit(handleSubmit)} className="space-y-4">
              <FormField
                control={form.control}
                name="label"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>{t('settings.apiKeys.keyLabel')}</FormLabel>
                    <FormControl>
                      <Input
                        placeholder={t('settings.apiKeys.keyLabelPlaceholder')}
                        {...field}
                      />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />

              <FormField
                control={form.control}
                name="expiry"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>{t('settings.apiKeys.keyExpiry')}</FormLabel>
                    <Select onValueChange={field.onChange} defaultValue={field.value}>
                      <FormControl>
                        <SelectTrigger>
                          <SelectValue />
                        </SelectTrigger>
                      </FormControl>
                      <SelectContent>
                        {expiryOptions.map((option) => (
                          <SelectItem key={option.value} value={option.value}>
                            {option.label}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                    <FormMessage />
                  </FormItem>
                )}
              />

              <DialogFooter>
                <Button
                  type="button"
                  variant="outline"
                  onClick={handleClose}
                  disabled={isLoading}
                >
                  {t('common.cancel')}
                </Button>
                <Button type="submit" disabled={isLoading}>
                  {isLoading ? t('common.loading') : t('settings.apiKeys.createKey')}
                </Button>
              </DialogFooter>
            </form>
          </Form>
        ) : (
          <div className="space-y-4">
            <div className="bg-warning/10 text-warning border-warning/20 flex gap-3 rounded-lg border p-4">
              <AlertTriangle className="h-5 w-5 shrink-0" />
              <p className="text-sm">{t('settings.apiKeys.copyKeyWarning')}</p>
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium">
                {t('settings.apiKeys.yourNewKey')}
              </label>
              <div className="flex gap-2">
                <Input
                  value={newToken}
                  readOnly
                  className="font-mono text-xs"
                  onClick={(e) => e.currentTarget.select()}
                />
                <Button
                  size="sm"
                  variant="outline"
                  onClick={handleCopy}
                  className="shrink-0"
                >
                  {copied ? (
                    <>
                      <Check className="mr-2 h-4 w-4" />
                      {t('settings.apiKeys.copied')}
                    </>
                  ) : (
                    <>
                      <Copy className="mr-2 h-4 w-4" />
                      {t('settings.apiKeys.copy')}
                    </>
                  )}
                </Button>
              </div>
            </div>

            <DialogFooter>
              <Button onClick={handleClose}>{t('common.close')}</Button>
            </DialogFooter>
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
