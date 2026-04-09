import { useTranslation } from 'react-i18next'
import { useParams } from 'react-router-dom'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { ShieldCheck, CheckCircle, Info } from 'lucide-react'
import { toast } from 'sonner'
import { verificationRequestSchema, type VerificationRequestInput } from '@zako-ac/zako3-data'
import { useTap, useRequestVerification } from '@/features/taps'
import { Button } from '@/components/ui/button'
import {
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
    CardFooter,
} from '@/components/ui/card'
import {
    Form,
    FormControl,
    FormDescription,
    FormField,
    FormItem,
    FormLabel,
    FormMessage,
} from '@/components/ui/form'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Skeleton } from '@/components/ui/skeleton'

export const TapVerificationPage = () => {
    const { t } = useTranslation()
    const { tapId } = useParams<{ tapId: string }>()

    const { data: tap, isLoading } = useTap(tapId)
    const { mutateAsync: requestVerification, isPending: isSubmitting } = useRequestVerification()

    const form = useForm<VerificationRequestInput>({
        resolver: zodResolver(verificationRequestSchema),
        defaultValues: {
            title: '',
            description: '',
        },
    })

    const onSubmit = async (data: VerificationRequestInput) => {
        if (!tapId) return
        try {
            await requestVerification({ tapId, data })
            toast.success(t('taps.verification.requestSuccess'))
            form.reset()
        } catch (error) {
            toast.error(
                error instanceof Error ? error.message : 'Failed to submit verification request'
            )
        }
    }

    if (isLoading) {
        return (
            <div className="space-y-6">
                <div>
                    <Skeleton className="mb-2 h-8 w-48" />
                    <Skeleton className="h-4 w-96" />
                </div>
                <Card>
                    <CardHeader>
                        <Skeleton className="h-6 w-32" />
                        <Skeleton className="h-4 w-64" />
                    </CardHeader>
                    <CardContent className="space-y-4">
                        <Skeleton className="h-24 w-full" />
                    </CardContent>
                </Card>
            </div>
        )
    }

    if (!tap) {
        return (
            <div className="py-12 text-center">
                <p className="text-muted-foreground">{t('taps.notFound')}</p>
            </div>
        )
    }

    const renderStatus = () => {
        // Since we don't have the current verification status in the Tap object directly in the mock,
        // we'll check the occupation. If it's verified/official, it's already done.
        // For the "pending" state, we'd ideally need to fetch the user's requests for this tap.
        // For now, let's focus on the occupation and the form.
        
        if (tap.occupation === 'verified' || tap.occupation === 'official') {
            return (
                <Alert className="bg-success/10 border-success/20">
                    <CheckCircle className="h-4 w-4 text-success" />
                    <AlertTitle>{t('taps.occupations.' + tap.occupation)}</AlertTitle>
                    <AlertDescription>
                        {t('taps.verification.status.approved')}
                    </AlertDescription>
                </Alert>
            )
        }

        // Note: In a real app, we would fetch the last verification request for this tap.
        // If it's pending, we show the pending alert.
        return null
    }

    return (
        <div className="space-y-6">
            <div>
                <h1 className="text-2xl font-semibold">{t('taps.verification.title')}</h1>
                <p className="text-muted-foreground">{t('taps.verification.subtitle')}</p>
            </div>

            {renderStatus()}

            {tap.occupation === 'base' && (
                <Card>
                    <CardHeader>
                        <CardTitle className="flex items-center gap-2">
                            <ShieldCheck className="h-5 w-5" />
                            {t('taps.verification.formTitle')}
                        </CardTitle>
                        <CardDescription>
                            {t('taps.verification.status.none')}
                        </CardDescription>
                    </CardHeader>
                    <CardContent>
                        <Form {...form}>
                            <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
                                <FormField
                                    control={form.control}
                                    name="title"
                                    render={({ field }) => (
                                        <FormItem>
                                            <FormLabel>{t('taps.verification.titleLabel')}</FormLabel>
                                            <FormControl>
                                                <Input placeholder={t('taps.verification.titlePlaceholder')} {...field} />
                                            </FormControl>
                                            <FormMessage />
                                        </FormItem>
                                    )}
                                />

                                <FormField
                                    control={form.control}
                                    name="description"
                                    render={({ field }) => (
                                        <FormItem>
                                            <FormLabel>{t('taps.verification.description')}</FormLabel>
                                            <FormControl>
                                                <Textarea 
                                                    placeholder={t('taps.verification.descriptionPlaceholder')} 
                                                    className="min-h-32 resize-none"
                                                    {...field} 
                                                />
                                            </FormControl>
                                            <FormDescription>
                                                {t('taps.verification.descriptionHelp')}
                                            </FormDescription>
                                            <FormMessage />
                                        </FormItem>
                                    )}
                                />

                                <div className="flex justify-end">
                                    <Button type="submit" disabled={isSubmitting}>
                                        {isSubmitting ? t('common.loading') : t('common.confirm')}
                                    </Button>
                                </div>
                            </form>
                        </Form>
                    </CardContent>
                    <CardFooter className="bg-muted/50 border-t px-6 py-4">
                        <div className="flex items-start gap-2 text-sm text-muted-foreground">
                            <Info className="h-4 w-4 mt-0.5 shrink-0" />
                            <p>
                                {t('taps.verification.verificationInfo')}
                            </p>
                        </div>
                    </CardFooter>
                </Card>
            )}
        </div>
    )
}
