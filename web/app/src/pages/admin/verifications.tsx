import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router-dom'
import { ShieldCheck, CheckCircle, XCircle, Eye } from 'lucide-react'
import { toast } from 'sonner'
import {
    useVerificationRequests,
    useApproveVerification,
    useRejectVerification,
} from '@/features/admin'
import { usePagination } from '@/hooks'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Skeleton } from '@/components/ui/skeleton'
import {
    Table,
    TableBody,
    TableCell,
    TableHead,
    TableHeader,
    TableRow,
} from '@/components/ui/table'
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from '@/components/ui/select'
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
} from '@/components/ui/dialog'
import { Textarea } from '@/components/ui/textarea'
import { Label } from '@/components/ui/label'
import { ConfirmDialog, DataPagination } from '@/components/common'
import { formatRelativeTime } from '@/lib/date'
import { ROUTES } from '@/lib/constants'
import type { VerificationRequestFull, VerificationStatus } from '@zako-ac/zako3-data'

export const AdminVerificationsPage = () => {
    const { t, i18n } = useTranslation()
    const { pagination, setPage, setPerPage, getPaginationInfo } = usePagination()

    // Filter state
    const [statusFilter, setStatusFilter] = useState<VerificationStatus | 'all'>(
        'all'
    )

    // Dialog state
    const [viewDialogOpen, setViewDialogOpen] = useState(false)
    const [approveDialogOpen, setApproveDialogOpen] = useState(false)
    const [rejectDialogOpen, setRejectDialogOpen] = useState(false)
    const [selectedRequest, setSelectedRequest] =
        useState<VerificationRequestFull | null>(null)
    const [rejectionReason, setRejectionReason] = useState('')

    const { data, isLoading } = useVerificationRequests({
        page: pagination.page,
        perPage: pagination.perPage,
        status: statusFilter === 'all' ? undefined : statusFilter,
    })

    const { mutateAsync: approveVerification, isPending: isApproving } =
        useApproveVerification()
    const { mutateAsync: rejectVerification, isPending: isRejecting } =
        useRejectVerification()

    const requests = data?.data ?? []
    const paginationInfo = getPaginationInfo(data?.meta)

    const handleViewClick = (request: VerificationRequestFull) => {
        setSelectedRequest(request)
        setViewDialogOpen(true)
    }

    const handleApproveClick = (request: VerificationRequestFull) => {
        setSelectedRequest(request)
        setApproveDialogOpen(true)
    }

    const handleRejectClick = (request: VerificationRequestFull) => {
        setSelectedRequest(request)
        setRejectionReason('')
        setRejectDialogOpen(true)
    }

    const handleApprove = async () => {
        if (!selectedRequest) return
        try {
            await approveVerification(selectedRequest.id)
            toast.success(t('admin.verifications.approveSuccess'))
            setApproveDialogOpen(false)
            setViewDialogOpen(false)
            setSelectedRequest(null)
        } catch (error) {
            toast.error(
                error instanceof Error
                    ? error.message
                    : 'Failed to approve verification'
            )
        }
    }

    const handleReject = async () => {
        if (!selectedRequest || !rejectionReason.trim()) {
            toast.error(t('admin.verifications.rejectionReasonRequired'))
            return
        }
        try {
            await rejectVerification({
                requestId: selectedRequest.id,
                reason: rejectionReason,
            })
            toast.success(t('admin.verifications.rejectSuccess'))
            setRejectDialogOpen(false)
            setSelectedRequest(null)
            setViewDialogOpen(false)
            setRejectionReason('')
        } catch (error) {
            toast.error(
                error instanceof Error ? error.message : 'Failed to reject verification'
            )
        }
    }


    const getStatusBadge = (status: VerificationStatus) => {
        switch (status) {
            case 'pending':
                return (
                    <Badge variant="secondary">
                        {t('admin.verifications.statusPending')}
                    </Badge>
                )
            case 'approved':
                return (
                    <Badge
                        variant="default"
                        className="bg-success text-success-foreground"
                    >
                        <CheckCircle className="mr-1 h-3 w-3" />
                        {t('admin.verifications.statusApproved')}
                    </Badge>
                )
            case 'rejected':
                return (
                    <Badge variant="destructive">
                        <XCircle className="mr-1 h-3 w-3" />
                        {t('admin.verifications.statusRejected')}
                    </Badge>
                )
        }
    }

    return (
        <div className="space-y-6">
            <div>
                <h1 className="flex items-center gap-2 text-2xl font-semibold">
                    <ShieldCheck className="h-6 w-6" />
                    {t('admin.verifications.title')}
                </h1>
                <p className="text-muted-foreground">
                    {t('admin.verifications.subtitle')}
                </p>
            </div>

            {/* Filters */}
            <div className="flex items-center gap-4">
                <div className="flex items-center gap-2">
                    <Label>{t('admin.verifications.filterByStatus')}</Label>
                    <Select
                        value={statusFilter}
                        onValueChange={(value) =>
                            setStatusFilter(value as VerificationStatus | 'all')
                        }
                    >
                        <SelectTrigger className="w-40">
                            <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                            <SelectItem value="all">
                                {t('admin.verifications.statusAll')}
                            </SelectItem>
                            <SelectItem value="pending">
                                {t('admin.verifications.statusPending')}
                            </SelectItem>
                            <SelectItem value="approved">
                                {t('admin.verifications.statusApproved')}
                            </SelectItem>
                            <SelectItem value="rejected">
                                {t('admin.verifications.statusRejected')}
                            </SelectItem>
                        </SelectContent>
                    </Select>
                </div>
            </div>

            {/* Table */}
            {isLoading ? (
                <div className="space-y-2">
                    {Array.from({ length: 5 }).map((_, i) => (
                        <Skeleton key={i} className="h-16 w-full" />
                    ))}
                </div>
            ) : requests.length === 0 ? (
                <div className="rounded-lg border border-dashed p-12 text-center">
                    <ShieldCheck className="text-muted-foreground mx-auto mb-4 h-12 w-12" />
                    <h3 className="text-lg font-semibold">
                        {t('admin.verifications.noRequests')}
                    </h3>
                    <p className="text-muted-foreground">
                        {t('admin.verifications.noRequestsDescription')}
                    </p>
                </div>
            ) : (
                <>
                    <div className="rounded-lg border">
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHead>{t('admin.verifications.tapName')}</TableHead>
                                    <TableHead>{t('admin.verifications.title')}</TableHead>
                                    <TableHead>{t('admin.verifications.status')}</TableHead>
                                    <TableHead>{t('admin.verifications.requestedAt')}</TableHead>
                                    <TableHead className="text-right">
                                        {t('common.actions')}
                                    </TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {requests.map((request) => (
                                    <TableRow key={request.id}>
                                        <TableCell>
                                            <Link
                                                to={ROUTES.ADMIN_TAP(request.tapId)}
                                                className="font-medium hover:underline"
                                            >
                                                {request.tap?.name || request.tapId}
                                            </Link>
                                        </TableCell>
                                        <TableCell className="max-w-md">
                                            <p className="font-medium">{request.title}</p>
                                        </TableCell>
                                        <TableCell>{getStatusBadge(request.status)}</TableCell>
                                        <TableCell className="text-muted-foreground text-sm">
                                            {formatRelativeTime(request.createdAt, i18n.language)}
                                        </TableCell>
                                        <TableCell className="text-right">
                                            <div className="flex justify-end gap-2">
                                                <Button
                                                    size="sm"
                                                    variant="outline"
                                                    onClick={() => handleViewClick(request)}
                                                >
                                                    <Eye className="mr-1 h-3 w-3" />
                                                    {t('admin.verifications.view')}
                                                </Button>
                                            </div>
                                        </TableCell>
                                    </TableRow>
                                ))}
                            </TableBody>
                        </Table>
                    </div>

                    {data?.meta && paginationInfo.totalPages > 1 && (
                        <DataPagination
                            meta={data.meta}
                            onPageChange={setPage}
                            onPerPageChange={setPerPage}
                        />
                    )}
                </>
            )}

            {/* Details Modal */}
            <Dialog open={viewDialogOpen} onOpenChange={setViewDialogOpen}>
                <DialogContent className="max-w-2xl">
                    <DialogHeader>
                        <DialogTitle>
                            {t('admin.verifications.viewDetails')}
                        </DialogTitle>
                        <DialogDescription>
                            {selectedRequest?.tap?.name || selectedRequest?.tapId}
                        </DialogDescription>
                    </DialogHeader>
                    <div className="space-y-4 py-4">
                        <div className="space-y-2">
                            <h4 className="text-sm font-medium">{t('admin.verifications.title')}</h4>
                            <p className="text-sm">{selectedRequest?.title}</p>
                        </div>
                        <div className="space-y-2">
                            <h4 className="text-sm font-medium">{t('taps.form.description')}</h4>
                            <div className="bg-muted rounded-md p-4">
                                <p className="whitespace-pre-wrap text-sm leading-relaxed">
                                    {selectedRequest?.description}
                                </p>
                            </div>
                        </div>
                        {selectedRequest?.status === 'rejected' && selectedRequest?.rejectionReason && (
                            <div className="space-y-2">
                                <h4 className="text-sm font-medium text-destructive">{t('admin.verifications.rejectionReasonLabel')}</h4>
                                <div className="bg-destructive/10 border border-destructive/20 rounded-md p-4">
                                    <p className="text-sm leading-relaxed">
                                        {selectedRequest.rejectionReason}
                                    </p>
                                </div>
                            </div>
                        )}
                    </div>
                    <DialogFooter>
                        <div className="flex w-full items-center justify-between">
                            <div className="flex gap-2">
                                {selectedRequest?.status === 'pending' && (
                                    <>
                                        <Button
                                            variant="default"
                                            onClick={() => handleApproveClick(selectedRequest)}
                                        >
                                            <CheckCircle className="mr-1 h-3 w-3" />
                                            {t('admin.verifications.approve')}
                                        </Button>
                                        <Button
                                            variant="outline"
                                            onClick={() => handleRejectClick(selectedRequest)}
                                        >
                                            <XCircle className="mr-1 h-3 w-3" />
                                            {t('admin.verifications.reject')}
                                        </Button>
                                    </>
                                )}
                            </div>
                            <Button
                                variant="outline"
                                onClick={() => setViewDialogOpen(false)}
                            >
                                {t('common.close')}
                            </Button>
                        </div>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            {/* Approve Confirmation Dialog */}
            <ConfirmDialog
                open={approveDialogOpen}
                onOpenChange={setApproveDialogOpen}
                title={t('admin.verifications.approveConfirmTitle')}
                description={t('admin.verifications.approveConfirmDescription', {
                    name: selectedRequest?.tap?.name || selectedRequest?.tapId,
                })}
                confirmLabel={t('admin.verifications.approve')}
                onConfirm={handleApprove}
                isLoading={isApproving}
                variant="default"
            />

            {/* Reject Dialog with Reason Input */}
            <Dialog open={rejectDialogOpen} onOpenChange={setRejectDialogOpen}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>
                            {t('admin.verifications.rejectConfirmTitle')}
                        </DialogTitle>
                        <DialogDescription>
                            {t('admin.verifications.rejectConfirmDescription', {
                                name: selectedRequest?.tap?.name || selectedRequest?.tapId,
                            })}
                        </DialogDescription>
                    </DialogHeader>
                    <div className="space-y-4 py-4">
                        <div className="space-y-2">
                            <Label htmlFor="rejection-reason">
                                {t('admin.verifications.rejectionReasonLabel')}
                            </Label>
                            <Textarea
                                id="rejection-reason"
                                placeholder={t(
                                    'admin.verifications.rejectionReasonPlaceholder'
                                )}
                                value={rejectionReason}
                                onChange={(e) => setRejectionReason(e.target.value)}
                                rows={4}
                            />
                        </div>
                    </div>
                    <DialogFooter>
                        <Button
                            variant="outline"
                            onClick={() => setRejectDialogOpen(false)}
                            disabled={isRejecting}
                        >
                            {t('common.cancel')}
                        </Button>
                        <Button
                            variant="destructive"
                            onClick={handleReject}
                            disabled={isRejecting || !rejectionReason.trim()}
                        >
                            {isRejecting
                                ? t('common.loading')
                                : t('admin.verifications.reject')}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </div>
    )
}
