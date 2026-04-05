import { useState } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { useTranslation } from 'react-i18next'
import { useNavigate, useParams } from 'react-router-dom'
import { toast } from 'sonner'
import { AlertTriangle, Copy } from 'lucide-react'
import { updateTapSchema, type UpdateTapInput } from '@zako-ac/zako3-data'
import {
    useTap,
    useUpdateTap,
    useDeleteTap,
} from '@/features/taps'
import { TAP_PERMISSIONS, TAP_ROLES, ROUTES } from '@/lib/constants'
import { Button } from '@/components/ui/button'
import {
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
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
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from '@/components/ui/select'
import { Checkbox } from '@/components/ui/checkbox'
import { Skeleton } from '@/components/ui/skeleton'
import { ConfirmDialog } from '@/components/common'
import { UserListSelector } from '@/components/tap/user-list-selector'
import type { TapRole } from '@zako-ac/zako3-data'

export const TapSettingsPage = () => {
    const { t } = useTranslation()
    const navigate = useNavigate()
    const { tapId } = useParams<{ tapId: string }>()
    const [deleteDialogOpen, setDeleteDialogOpen] = useState(false)

    const { data: tap, isLoading } = useTap(tapId)
    const { mutateAsync: updateTap, isPending: isUpdating } = useUpdateTap(tapId!)
    const { mutateAsync: deleteTap, isPending: isDeleting } = useDeleteTap()

    const form = useForm({
        resolver: zodResolver(updateTapSchema),
        values: tap
            ? {
                name: tap.name,
                description: tap.description,
                roles: tap.roles,
                permission: tap.permission,
            }
            : undefined,
    })

    const onSubmit = async (data: UpdateTapInput) => {
        try {
            await updateTap(data)
            toast.success(t('taps.settings.updateSuccess'))
        } catch (error) {
            toast.error(
                error instanceof Error ? error.message : 'Failed to update tap'
            )
        }
    }

    const handleDelete = async () => {
        if (!tapId) return
        try {
            await deleteTap(tapId)
            toast.success(t('taps.deleteSuccess'))
            navigate(ROUTES.TAPS_MINE)
        } catch (error) {
            toast.error(
                error instanceof Error ? error.message : 'Failed to delete tap'
            )
        }
    }

    if (isLoading) {
        return (
            <div className="mx-auto max-w-2xl space-y-6">
                <div>
                    <Skeleton className="mb-2 h-8 w-48" />
                    <Skeleton className="h-4 w-96" />
                </div>
                <Card>
                    <CardHeader>
                        <Skeleton className="h-6 w-32" />
                    </CardHeader>
                    <CardContent className="space-y-4">
                        <Skeleton className="h-10 w-full" />
                        <Skeleton className="h-10 w-full" />
                    </CardContent>
                </Card>
            </div>
        )
    }

    if (!tap) {
        return (
            <div className="py-12 text-center">
                <p className="text-muted-foreground">Tap not found</p>
            </div>
        )
    }

    return (
        <div className="mx-auto max-w-2xl space-y-6">
            <div>
                <h1 className="text-2xl font-semibold">{t('taps.settings.title')}</h1>
                <p className="text-muted-foreground">{t('taps.settings.subtitle')}</p>
            </div>

            <Form {...form}>
                <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-6">
                    <Card>
                        <CardHeader>
                            <CardTitle>{t('taps.settings.basic')}</CardTitle>
                            <CardDescription>
                                Edit the basic information for your tap
                            </CardDescription>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            <div className="space-y-2">
                                <FormLabel>{t('taps.form.id')}</FormLabel>
                                <div className="flex items-center gap-2">
                                    <Input value={tap.id} className="font-mono bg-muted text-muted-foreground flex-1" disabled />
                                    <Button
                                        type="button"
                                        variant="outline"
                                        size="icon"
                                        onClick={() => {
                                            navigator.clipboard.writeText(tap.id)
                                            toast.success(t('common.copied'))
                                        }}
                                        title={t('common.copy')}
                                    >
                                        <Copy className="h-4 w-4" />
                                    </Button>
                                </div>
                                <FormDescription className="text-muted-foreground">
                                    The Tap ID cannot be changed.
                                </FormDescription>
                            </div>

                            <FormField
                                control={form.control}
                                name="name"
                                render={({ field }) => (
                                    <FormItem>
                                        <FormLabel>{t('taps.form.name')}</FormLabel>
                                        <FormControl>
                                            <Input {...field} />
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
                                        <FormLabel>{t('taps.form.description')}</FormLabel>
                                        <FormControl>
                                            <Textarea className="min-h-24 resize-none" {...field} />
                                        </FormControl>
                                        <FormMessage />
                                    </FormItem>
                                )}
                            />
                        </CardContent>
                    </Card>

                    <Card>
                        <CardHeader>
                            <CardTitle>{t('taps.settings.rolesAndPermissions')}</CardTitle>
                            <CardDescription>
                                Configure how users can interact with your tap
                            </CardDescription>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            <FormField
                                control={form.control}
                                name="roles"
                                render={() => (
                                    <FormItem>
                                        <div className="mb-4">
                                            <FormLabel>{t('taps.form.roles')}</FormLabel>
                                            <FormDescription>
                                                {t('taps.form.rolesHelp')}
                                            </FormDescription>
                                        </div>
                                        <div className="space-y-2">
                                            {TAP_ROLES.map((role) => (
                                                <FormField
                                                    key={role}
                                                    control={form.control}
                                                    name="roles"
                                                    render={({ field }) => {
                                                        return (
                                                            <FormItem className="flex flex-row items-start space-y-0 space-x-3">
                                                                <FormControl>
                                                                    <Checkbox
                                                                        value={role}
                                                                        checked={field.value?.includes(role)}
                                                                        onCheckedChange={(checked) => {
                                                                            return checked
                                                                                ? field.onChange([
                                                                                    ...field.value!,
                                                                                    role,
                                                                                ])
                                                                                : field.onChange(
                                                                                    field.value?.filter(
                                                                                        (value: TapRole) => value !== role
                                                                                    )
                                                                                )
                                                                        }}
                                                                    />
                                                                </FormControl>
                                                                <FormLabel className="font-normal">
                                                                    {t(`taps.roleLabels.${role}`)}
                                                                </FormLabel>
                                                            </FormItem>
                                                        )
                                                    }}
                                                />
                                            ))}
                                        </div>
                                        <FormMessage />
                                    </FormItem>
                                )}
                            />

                            <FormField
                                control={form.control}
                                name="permission"
                                render={({ field }) => (
                                    <FormItem>
                                        <FormLabel>{t('taps.form.permission')}</FormLabel>
                                        <Select
                                            onValueChange={(value) => {
                                                if (value === 'owner_only') {
                                                    field.onChange({ type: 'owner_only' })
                                                } else if (value === 'public') {
                                                    field.onChange({ type: 'public' })
                                                } else if (value === 'whitelisted') {
                                                    const currentUserIds =
                                                        field.value?.type === 'whitelisted'
                                                            ? field.value.userIds
                                                            : []
                                                    field.onChange({
                                                        type: 'whitelisted',
                                                        userIds: currentUserIds,
                                                    })
                                                } else if (value === 'blacklisted') {
                                                    const currentUserIds =
                                                        field.value?.type === 'blacklisted'
                                                            ? field.value.userIds
                                                            : []
                                                    field.onChange({
                                                        type: 'blacklisted',
                                                        userIds: currentUserIds,
                                                    })
                                                }
                                            }}
                                            value={field.value?.type}
                                        >
                                            <FormControl>
                                                <SelectTrigger>
                                                    <SelectValue />
                                                </SelectTrigger>
                                            </FormControl>
                                            <SelectContent>
                                                {TAP_PERMISSIONS.map((permission) => (
                                                    <SelectItem key={permission} value={permission}>
                                                        {t(`taps.permissions.${permission}`)}
                                                    </SelectItem>
                                                ))}
                                            </SelectContent>
                                        </Select>
                                        <FormMessage />
                                    </FormItem>
                                )}
                            />

                            {form.watch('permission')?.type === 'whitelisted' && (
                                <FormField
                                    control={form.control}
                                    name="permission"
                                    render={({ field }) => (
                                        <FormItem>
                                            <UserListSelector
                                                value={
                                                    field.value && field.value.type === 'whitelisted'
                                                        ? field.value.userIds
                                                        : []
                                                }
                                                onChange={(userIds) =>
                                                    field.onChange({ type: 'whitelisted', userIds })
                                                }
                                                label={t('taps.form.whitelistedUsers')}
                                                placeholder={t('taps.form.addWhitelistedUsers')}
                                                description={t('taps.form.whitelistedUsersHelp')}
                                            />
                                            <FormMessage />
                                        </FormItem>
                                    )}
                                />
                            )}

                            {form.watch('permission')?.type === 'blacklisted' && (
                                <FormField
                                    control={form.control}
                                    name="permission"
                                    render={({ field }) => (
                                        <FormItem>
                                            <UserListSelector
                                                value={
                                                    field.value && field.value.type === 'blacklisted'
                                                        ? field.value.userIds
                                                        : []
                                                }
                                                onChange={(userIds) =>
                                                    field.onChange({ type: 'blacklisted', userIds })
                                                }
                                                label={t('taps.form.blacklistedUsers')}
                                                placeholder={t('taps.form.addBlacklistedUsers')}
                                                description={t('taps.form.blacklistedUsersHelp')}
                                            />
                                            <FormMessage />
                                        </FormItem>
                                    )}
                                />
                            )}
                        </CardContent>
                    </Card>

                    <Card className="border-destructive/50">
                        <CardHeader>
                            <CardTitle className="text-destructive flex items-center gap-2">
                                <AlertTriangle className="h-5 w-5" />
                                {t('taps.settings.dangerZone')}
                            </CardTitle>
                        </CardHeader>
                        <CardContent>
                            <p className="text-muted-foreground mb-4 text-sm">
                                {t('taps.settings.deleteWarning')}
                            </p>
                            <Button
                                type="button"
                                variant="destructive"
                                onClick={() => setDeleteDialogOpen(true)}
                            >
                                {t('common.delete')}
                            </Button>
                        </CardContent>
                    </Card>

                    <div className="flex justify-end gap-4">
                        <Button
                            type="button"
                            variant="outline"
                            onClick={() => navigate(ROUTES.TAPS_MINE)}
                        >
                            {t('common.cancel')}
                        </Button>
                        <Button type="submit" disabled={isUpdating}>
                            {isUpdating ? t('common.loading') : t('common.confirm')}
                        </Button>
                    </div>
                </form>
            </Form>

            <ConfirmDialog
                open={deleteDialogOpen}
                onOpenChange={setDeleteDialogOpen}
                title={t('taps.deleteConfirmTitle')}
                description={t('taps.deleteConfirmDescription', { name: tap.name })}
                confirmLabel={t('common.delete')}
                onConfirm={handleDelete}
                isLoading={isDeleting}
                variant="destructive"
            />
        </div>
    )
}
