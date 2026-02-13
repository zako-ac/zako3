import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
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
import { Textarea } from '@/components/ui/textarea'
import { reportTapSchema, type ReportTapInput } from '@/features/taps'

interface ReportModalProps {
    open: boolean
    onOpenChange: (open: boolean) => void
    tapName: string
    tapId?: string
    onSubmit: (data: ReportTapInput) => Promise<void>
}

const reportReasons = [
    { value: 'inappropriate', labelKey: 'taps.report.reasons.inappropriate' },
    { value: 'spam', labelKey: 'taps.report.reasons.spam' },
    { value: 'copyright', labelKey: 'taps.report.reasons.copyright' },
    { value: 'other', labelKey: 'taps.report.reasons.other' },
] as const

export const ReportModal = ({
    open,
    onOpenChange,
    tapName,
    onSubmit,
}: ReportModalProps) => {
    const { t } = useTranslation()
    const [isSubmitting, setIsSubmitting] = useState(false)

    const form = useForm<ReportTapInput>({
        resolver: zodResolver(reportTapSchema),
        defaultValues: {
            reason: 'inappropriate',
            description: '',
        },
    })

    const handleSubmit = async (data: ReportTapInput) => {
        setIsSubmitting(true)
        try {
            await onSubmit(data)
            form.reset()
            onOpenChange(false)
        } finally {
            setIsSubmitting(false)
        }
    }

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>{t('taps.report.title')}</DialogTitle>
                    <DialogDescription>
                        Reporting tap: <strong>{tapName}</strong>
                    </DialogDescription>
                </DialogHeader>

                <Form {...form}>
                    <form onSubmit={form.handleSubmit(handleSubmit)} className="space-y-4">
                        <FormField
                            control={form.control}
                            name="reason"
                            render={({ field }) => (
                                <FormItem>
                                    <FormLabel>{t('taps.report.reason')}</FormLabel>
                                    <Select onValueChange={field.onChange} defaultValue={field.value}>
                                        <FormControl>
                                            <SelectTrigger>
                                                <SelectValue />
                                            </SelectTrigger>
                                        </FormControl>
                                        <SelectContent>
                                            {reportReasons.map((reason) => (
                                                <SelectItem key={reason.value} value={reason.value}>
                                                    {t(reason.labelKey)}
                                                </SelectItem>
                                            ))}
                                        </SelectContent>
                                    </Select>
                                    <FormMessage />
                                </FormItem>
                            )}
                        />

                        <FormField
                            control={form.control}
                            name="description"
                            render={({ field }) => (
                                <FormItem>
                                    <FormLabel>{t('taps.report.description')}</FormLabel>
                                    <FormControl>
                                        <Textarea
                                            {...field}
                                            placeholder="Please provide details about the issue..."
                                            rows={4}
                                        />
                                    </FormControl>
                                    <FormMessage />
                                </FormItem>
                            )}
                        />

                        <DialogFooter>
                            <Button
                                type="button"
                                variant="outline"
                                onClick={() => onOpenChange(false)}
                                disabled={isSubmitting}
                            >
                                {t('common.cancel')}
                            </Button>
                            <Button type="submit" disabled={isSubmitting}>
                                {isSubmitting ? t('common.loading') : t('taps.report.submit')}
                            </Button>
                        </DialogFooter>
                    </form>
                </Form>
            </DialogContent>
        </Dialog>
    )
}
