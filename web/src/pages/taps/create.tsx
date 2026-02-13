import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { createTapSchema, type CreateTapInput } from '@zako-ac/zako3-data'
import { useCreateTap } from '@/features/taps'
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
import { UserListSelector } from '@/components/tap/user-list-selector'
import type { TapRole } from '@zako-ac/zako3-data'

export const CreateTapPage = () => {
    const { t } = useTranslation()
    const navigate = useNavigate()
    const { mutateAsync: createTap, isPending } = useCreateTap()

    const form = useForm({
        resolver: zodResolver(createTapSchema),
        defaultValues: {
            id: '',
            name: '',
            description: '',
            roles: [] as TapRole[],
            permission: { type: 'owner_only' as const },
        },
    })

    const onSubmit = async (data: CreateTapInput) => {
        try {
            const tap = await createTap(data)
            toast.success(t('taps.createSuccess'))
            navigate(ROUTES.TAP_STATS(tap.id))
        } catch (error) {
            toast.error(
                error instanceof Error ? error.message : 'Failed to create tap'
            )
        }
    }

    return (
        <div className="mx-auto max-w-2xl space-y-6">
            <div>
                <h1 className="text-2xl font-semibold">{t('taps.create')}</h1>
                <p className="text-muted-foreground">
                    {t('taps.createFirstDescription')}
                </p>
            </div>

            <Form {...form}>
                <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-6">
                    <Card>
                        <CardHeader>
                            <CardTitle>{t('taps.settings.basic')}</CardTitle>
                            <CardDescription>
                                Set up the basic information for your tap
                            </CardDescription>
                        </CardHeader>
                        <CardContent className="space-y-4">
                            <FormField
                                control={form.control}
                                name="id"
                                render={({ field }) => (
                                    <FormItem>
                                        <FormLabel>{t('taps.form.id')}</FormLabel>
                                        <FormControl>
                                            <Input
                                                placeholder={t('taps.form.idPlaceholder')}
                                                {...field}
                                                className="font-mono"
                                            />
                                        </FormControl>
                                        <FormDescription>{t('taps.form.idHelp')}</FormDescription>
                                        <FormMessage />
                                    </FormItem>
                                )}
                            />

                            <FormField
                                control={form.control}
                                name="name"
                                render={({ field }) => (
                                    <FormItem>
                                        <FormLabel>{t('taps.form.name')}</FormLabel>
                                        <FormControl>
                                            <Input
                                                placeholder={t('taps.form.namePlaceholder')}
                                                {...field}
                                            />
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
                                            <Textarea
                                                placeholder={t('taps.form.descriptionPlaceholder')}
                                                className="min-h-24 resize-none"
                                                {...field}
                                            />
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
                                                                        checked={field.value?.includes(role)}
                                                                        onCheckedChange={(checked) => {
                                                                            return checked
                                                                                ? field.onChange([...field.value, role])
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
                                                    field.onChange({ type: 'whitelisted', userIds: [] })
                                                } else if (value === 'blacklisted') {
                                                    field.onChange({ type: 'blacklisted', userIds: [] })
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

                    <div className="flex justify-end gap-4">
                        <Button
                            type="button"
                            variant="outline"
                            onClick={() => navigate(ROUTES.TAPS_MINE)}
                        >
                            {t('common.cancel')}
                        </Button>
                        <Button type="submit" disabled={isPending}>
                            {isPending ? t('common.loading') : t('taps.create')}
                        </Button>
                    </div>
                </form>
            </Form>
        </div>
    )
}
