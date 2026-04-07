import { z } from 'zod';

export const guildSummarySchema = z.object({
    guildId: z.string(),
    guildName: z.string(),
    guildIconUrl: z.string().optional(),
    activeChannelId: z.string().optional(),
    activeChannelName: z.string().optional(),
    canManage: z.boolean(),
});

export type GuildSummaryDto = z.infer<typeof guildSummarySchema>;
