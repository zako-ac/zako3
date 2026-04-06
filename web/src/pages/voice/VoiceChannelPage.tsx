import { useParams } from 'react-router-dom'
import { SkipForward, Square, Undo2 } from 'lucide-react'
import { toast } from 'sonner'
import {
    usePlaybackState,
    usePlaybackHistory,
    useStopTrack,
    useSkipMusic,
    useEditQueue,
    useUndoAction,
    usePlaybackEvents,
} from '@/features/playback'
import type { TrackDto, AudioMetadataDto } from '@/features/playback'
import { CopyableId } from '@/components/tap/copyable-id'
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
    const { guildId, channelId } = useParams<{
        guildId: string
        channelId: string
    }>()

    usePlaybackEvents()

    const { data: states, isLoading: stateLoading } = usePlaybackState()
    const { data: history, isLoading: historyLoading } = usePlaybackHistory()

    const { mutate: stopTrack, isPending: isStopping } = useStopTrack()
    const { mutate: skipMusic, isPending: isSkipping } = useSkipMusic()
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
                <h2 className="text-lg font-medium">Queues</h2>
                {stateLoading ? (
                    <div className="space-y-3">
                        <Skeleton className="h-24 w-full" />
                        <Skeleton className="h-24 w-full" />
                    </div>
                ) : !channelState ? (
                    <p className="text-muted-foreground text-sm">
                        No active session in this channel.
                    </p>
                ) : (
                    Object.entries(channelState.queues as Record<string, TrackDto[]>).map(
                        ([queueName, tracks]) => (
                            <Card key={queueName}>
                                <CardHeader className="flex flex-row items-center justify-between pb-3">
                                    <CardTitle className="text-base font-medium capitalize">
                                        {queueName}
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
                                            Skip
                                        </Button>
                                    )}
                                </CardHeader>
                                <CardContent className="space-y-3">
                                    {tracks.length === 0 ? (
                                        <p className="text-muted-foreground text-sm">
                                            No tracks in queue.
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
                                                        Tap: {track.tapName} · Requested by:
                                                        <CopyableId id={track.requestedBy} />
                                                    </p>
                                                    <div className="mt-2 flex items-center gap-2">
                                                        <span className="text-muted-foreground text-xs w-10">
                                                            Vol
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
                    )
                )}
            </div>

            {/* History */}
            <div className="space-y-4">
                <h2 className="text-lg font-medium">History</h2>
                {historyLoading ? (
                    <div className="space-y-2">
                        <Skeleton className="h-12 w-full" />
                        <Skeleton className="h-12 w-full" />
                        <Skeleton className="h-12 w-full" />
                    </div>
                ) : !history || history.length === 0 ? (
                    <p className="text-muted-foreground text-sm">
                        No recent actions.
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
