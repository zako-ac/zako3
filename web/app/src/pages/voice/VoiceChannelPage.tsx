import { useParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { SkipForward, Square, Undo2, Pause, Play, RefreshCw } from 'lucide-react'
import { toast } from 'sonner'
import {
    usePlaybackState,
    useRefreshPlaybackState,
    usePlaybackHistory,
    useStopTrack,
    useSkipMusic,
    usePauseTrack,
    useResumeTrack,
    useEditQueue,
    useUndoAction,
} from '@/features/playback'
import type { TrackDto, AudioMetadataDto } from '@/features/playback'
import { CopyableId } from '@/components/tap/copyable-id'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Slider } from '@/components/ui/slider'
import { Skeleton } from '@/components/ui/skeleton'
import { Badge } from '@/components/ui/badge'
import { formatRelativeTime } from '@/lib/date'

function getNonUrlMetadata(metadata: AudioMetadataDto[]): AudioMetadataDto[] {
    return metadata.filter(
        (m) => !/url/i.test(m.type) && !m.value.startsWith('http')
    )
}

export const VoiceChannelPage = () => {
    const { t } = useTranslation()
    const { guildId, channelId } = useParams<{
        guildId: string
        channelId: string
    }>()

    const { data: states, isLoading: stateLoading, isRefetching } = usePlaybackState()
    const { data: history, isLoading: historyLoading } = usePlaybackHistory()
    const refreshState = useRefreshPlaybackState()

    const { mutate: stopTrack, isPending: isStopping } = useStopTrack()
    const { mutate: skipMusic, isPending: isSkipping } = useSkipMusic()
    const { mutate: pauseTrack, isPending: isPausing } = usePauseTrack()
    const { mutate: resumeTrack, isPending: isResuming } = useResumeTrack()
    const { mutate: editQueue } = useEditQueue()
    const { mutate: undoAction, isPending: isUndoing } = useUndoAction()

    const channelState = states?.find(
        (s) => s.guildId === guildId && s.channelId === channelId
    )

    if (!guildId || !channelId) return null

    return (
        <div className="space-y-6">
            <div>
                <h1 className="text-2xl font-semibold">
                    {channelState?.channelName || `...${channelId.slice(-6)}`}
                </h1>
                <p className="text-muted-foreground text-sm">
                    {channelState?.guildName || `Server ...${guildId.slice(-6)}`}
                </p>
            </div>

            {/* Queues */}
            <div className="space-y-4">
                <div className="flex items-center justify-between">
                    <h2 className="text-lg font-medium">{t('voice.queues')}</h2>
                    <Button
                        size="sm"
                        variant="ghost"
                        onClick={refreshState}
                        disabled={isRefetching}
                    >
                        <RefreshCw className={`h-4 w-4${isRefetching ? ' animate-spin' : ''}`} />
                    </Button>
                </div>
                {stateLoading ? (
                    <div className="space-y-3">
                        <Skeleton className="h-24 w-full" />
                        <Skeleton className="h-24 w-full" />
                    </div>
                ) : !channelState ? (
                    <p className="text-muted-foreground text-sm">
                        {t('voice.noActiveSession')}
                    </p>
                ) : (
                    Object.entries(channelState.queues as Record<string, TrackDto[]>)
                        .filter(([, tracks]) => tracks.length > 0)
                        .sort(([nameA], [nameB]) => {
                            if (nameA === 'music') return -1
                            if (nameB === 'music') return 1
                            return nameA.localeCompare(nameB)
                        })
                        .map(
                        ([queueName, tracks]) => {
                            const queueUser = channelState.queueMeta[queueName]?.user
                            return (
                            <Card key={queueName}>
                                <CardHeader className="flex flex-row items-center justify-between pb-3">
                                    <CardTitle className="text-base font-medium flex items-center gap-2">
                                        {queueUser ? (
                                            <>
                                                <Avatar className="h-5 w-5">
                                                    <AvatarImage src={queueUser.avatarUrl ?? undefined} />
                                                    <AvatarFallback className="text-xs">{queueUser.name[0]}</AvatarFallback>
                                                </Avatar>
                                                {queueUser.name}
                                            </>
                                        ) : (
                                            <span className="capitalize">{queueName}</span>
                                        )}
                                    </CardTitle>
                                    {queueName === 'music' && (
                                        <Button
                                            size="sm"
                                            variant="outline"
                                            disabled={isSkipping}
                                            onClick={() =>
                                                skipMusic(
                                                    { guildId, channelId },
                                                    {
                                                        onSuccess: () => toast.success('Skipped'),
                                                        onError: () => toast.error('Failed to skip'),
                                                    }
                                                )
                                            }
                                        >
                                            <SkipForward className="mr-1.5 h-4 w-4" />
                                            {t('voice.skip')}
                                        </Button>
                                    )}
                                </CardHeader>
                                <CardContent className="space-y-3">
                                    {tracks.length === 0 ? (
                                        <p className="text-muted-foreground text-sm">
                                            {t('voice.noTracksInQueue')}
                                        </p>
                                    ) : (
                                        tracks.map((track) => (
                                            <div
                                                key={track.trackId}
                                                className="flex items-center gap-4 rounded-md border p-3"
                                            >
                                                <div className="min-w-0 flex-1">
                                                    <p className="truncate text-sm font-medium">
                                                        {getNonUrlMetadata(track.metadata).length > 0
                                                            ? getNonUrlMetadata(track.metadata)
                                                                  .map((m) => m.value)
                                                                  .join(' · ')
                                                            : track.audioRequestString}
                                                    </p>
                                                    <p className="text-muted-foreground flex items-center gap-1 text-xs">
                                                        {t('voice.tap')}: {track.tapId} · {t('voice.requestedBy')}:
                                                        <CopyableId id={track.requestedBy} />
                                                    </p>
                                                    <div className="mt-2 flex items-center gap-2">
                                                        <span className="text-muted-foreground text-xs w-10">
                                                            {t('voice.volume')}
                                                        </span>
                                                        <Slider
                                                            className="w-32"
                                                            min={0}
                                                            max={100}
                                                            defaultValue={[
                                                                Math.round(
                                                                    track.volume *
                                                                    100
                                                                ),
                                                            ]}
                                                            onValueCommit={([
                                                                vol,
                                                            ]) =>
                                                                editQueue(
                                                                    {
                                                                        guildId,
                                                                        channelId,
                                                                        operations: [
                                                                            {
                                                                                op: 'set_volume',
                                                                                trackId: track.trackId,
                                                                                volume: vol / 100,
                                                                            },
                                                                        ],
                                                                    },
                                                                    {
                                                                        onSuccess: () => toast.success('Volume updated'),
                                                                        onError: () => toast.error('Failed to update volume'),
                                                                    }
                                                                )
                                                            }
                                                        />
                                                        <span className="text-muted-foreground text-xs w-8">
                                                            {Math.round(
                                                                track.volume *
                                                                100
                                                            )}
                                                            %
                                                        </span>
                                                    </div>
                                                </div>
                                                <Button
                                                    size="sm"
                                                    variant="outline"
                                                    disabled={isPausing || isResuming}
                                                    onClick={() =>
                                                        track.paused
                                                            ? resumeTrack(
                                                                { guildId, channelId, trackId: track.trackId },
                                                                {
                                                                    onSuccess: () => toast.success('Resumed'),
                                                                    onError: () => toast.error('Failed to resume'),
                                                                }
                                                            )
                                                            : pauseTrack(
                                                                { guildId, channelId, trackId: track.trackId },
                                                                {
                                                                    onSuccess: () => toast.success('Paused'),
                                                                    onError: () => toast.error('Failed to pause'),
                                                                }
                                                            )
                                                    }
                                                >
                                                    {track.paused ? (
                                                        <Play className="h-4 w-4" />
                                                    ) : (
                                                        <Pause className="h-4 w-4" />
                                                    )}
                                                </Button>
                                                <Button
                                                    size="sm"
                                                    variant="outline"
                                                    disabled={isStopping}
                                                    onClick={() =>
                                                        stopTrack(
                                                            { guildId, channelId, trackId: track.trackId },
                                                            {
                                                                onSuccess: () => toast.success('Track stopped'),
                                                                onError: () => toast.error('Failed to stop track'),
                                                            }
                                                        )
                                                    }
                                                >
                                                    <Square className="h-4 w-4" />
                                                </Button>
                                            </div>
                                        ))
                                    )}
                                </CardContent>
                            </Card>
                        )
                        })
                )}
            </div>

            {/* History */}
            <div className="space-y-4">
                <h2 className="text-lg font-medium">{t('voice.history')}</h2>
                {historyLoading ? (
                    <div className="space-y-2">
                        <Skeleton className="h-12 w-full" />
                        <Skeleton className="h-12 w-full" />
                        <Skeleton className="h-12 w-full" />
                    </div>
                ) : !history || history.length === 0 ? (
                    <p className="text-muted-foreground text-sm">
                        {t('voice.noRecentActions')}
                    </p>
                ) : (
                    <Card>
                        <CardContent className="p-0">
                            <div className="divide-y">
                                {history.map((action) => {
                                    const canUndo =
                                        !action.undoneAt &&
                                        action.actionType !== 'edit_queue'
                                    return (
                                        <div
                                            key={action.id}
                                            className="flex items-center gap-4 px-4 py-3"
                                        >
                                            <Badge
                                                variant="outline"
                                                className="shrink-0 capitalize"
                                            >
                                                {action.actionType.replace(
                                                    '_',
                                                    ' '
                                                )}
                                            </Badge>
                                            <div className="text-muted-foreground flex min-w-0 flex-1 items-center gap-1 text-sm">
                                                <CopyableId id={action.actorDiscordUserId} />
                                                <span className="mx-1">·</span>
                                                <CopyableId id={action.channelId} />
                                            </div>
                                            <span className="text-muted-foreground shrink-0 text-xs">
                                                {formatRelativeTime(
                                                    action.createdAt
                                                )}
                                            </span>
                                            {action.undoneAt ? (
                                                <Badge
                                                    variant="secondary"
                                                    className="shrink-0"
                                                >
                                                    Undone
                                                </Badge>
                                            ) : (
                                                <Button
                                                    size="sm"
                                                    variant="ghost"
                                                    disabled={
                                                        !canUndo || isUndoing
                                                    }
                                                    onClick={() =>
                                                        undoAction(action.id, {
                                                            onSuccess: () => toast.success('Action undone'),
                                                            onError: () => toast.error('Failed to undo'),
                                                        })
                                                    }
                                                >
                                                    <Undo2 className="h-4 w-4" />
                                                </Button>
                                            )}
                                        </div>
                                    )
                                })}
                            </div>
                        </CardContent>
                    </Card>
                )}
            </div>
        </div>
    )
}
