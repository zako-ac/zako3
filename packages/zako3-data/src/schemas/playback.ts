import { z } from 'zod';

export const audioMetadataSchema = z.object({
    type: z.string(),
    value: z.string(),
});

export const trackSchema = z.object({
    trackId: z.string(),
    queueName: z.string(),
    metadata: z.array(audioMetadataSchema),
    tapName: z.string(),
    audioRequestString: z.string(),
    requestedBy: z.string(),
    volume: z.number(),
});

export const guildPlaybackStateSchema = z.object({
    guildId: z.string(),
    guildName: z.string().default(''),
    channelId: z.string(),
    channelName: z.string().default(''),
    queues: z.record(z.string(), z.array(trackSchema)),
});

export const playbackActionSchema = z.object({
    id: z.string(),
    actionType: z.string(),
    guildId: z.string(),
    channelId: z.string(),
    actorDiscordUserId: z.string(),
    createdAt: z.string().datetime(),
    undoneAt: z.string().datetime().nullable(),
    undoneByDiscordUserId: z.string().nullable(),
});

export const stopTrackSchema = z.object({
    guildId: z.string(),
    channelId: z.string(),
    trackId: z.string(),
});

export const skipSchema = z.object({
    guildId: z.string(),
    channelId: z.string(),
});

export const queueOperationSchema = z.object({
    op: z.enum(['remove', 'set_volume']),
    trackId: z.string(),
    targetIndex: z.number().optional(),
    volume: z.number().optional(),
});

export const editQueueSchema = z.object({
    guildId: z.string(),
    channelId: z.string(),
    operations: z.array(queueOperationSchema),
});

export type AudioMetadataDto = z.infer<typeof audioMetadataSchema>;
export type TrackDto = z.infer<typeof trackSchema>;
export type GuildPlaybackStateDto = z.infer<typeof guildPlaybackStateSchema>;
export type PlaybackActionDto = z.infer<typeof playbackActionSchema>;
export type StopTrackDto = z.infer<typeof stopTrackSchema>;
export type SkipDto = z.infer<typeof skipSchema>;
export type QueueOperationDto = z.infer<typeof queueOperationSchema>;
export type EditQueueDto = z.infer<typeof editQueueSchema>;
