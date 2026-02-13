import { z } from "zod";

/**
 * Settings scope type enum
 */
export const scopeTypeSchema = z.enum([
    "global",
    "guild",
    "user",
    "perGuildUser",
    "admin",
]);

export type ScopeType = z.infer<typeof scopeTypeSchema>;

/**
 * Settings kind enum
 */
export const settingsKindSchema = z.enum(["user", "guild", "admin"]);

export type SettingsKind = z.infer<typeof settingsKindSchema>;

/**
 * Source kind enum
 */
export const sourceKindSchema = z.enum(["default", "entry", "merged"]);

export type SourceKind = z.infer<typeof sourceKindSchema>;

/**
 * Setting source schema
 */
export const settingSourceSchema = z.object({
    kind: sourceKindSchema,
    scope: z
        .object({
            kind: settingsKindSchema,
            scope: z.string(),
            guildId: z.string().optional(),
            userId: z.string().optional(),
        })
        .optional(),
    isImportant: z.boolean().optional(),
    scopeCount: z.number().optional(),
});

export type SettingSource = z.infer<typeof settingSourceSchema>;

/**
 * Setting value response schema
 */
export const settingValueSchema = z.object({
    value: z.any().describe("The setting value"),
    source: settingSourceSchema.describe("Source of the setting value"),
});

export type SettingValue = z.infer<typeof settingValueSchema>;

/**
 * Setting entry schema
 */
export const settingEntrySchema = z.object({
    keyId: z.string().describe("Setting key identifier"),
    value: z.any().describe("Setting value"),
    scope: z
        .object({
            kind: settingsKindSchema,
            type: z.string(),
            guildId: z.string().optional(),
            userId: z.string().optional(),
        })
        .describe("Setting scope"),
    isDefault: z.boolean().describe("Whether this is the default value"),
});

export type SettingEntry = z.infer<typeof settingEntrySchema>;

/**
 * Get setting query parameters
 */
export const getSettingQuerySchema = z.object({
    userId: z.string().optional().describe("User ID for user-scoped settings"),
    guildId: z
        .string()
        .optional()
        .describe("Guild ID for guild-scoped settings"),
});

export type GetSettingQuery = z.infer<typeof getSettingQuerySchema>;

/**
 * Set setting request schema
 */
export const setSettingRequestSchema = z.object({
    value: z.any().describe("The value to set"),
    scopeType: scopeTypeSchema.describe("The scope type to set the value at"),
    userId: z.string().optional().describe("User ID for user-scoped settings"),
    guildId: z
        .string()
        .optional()
        .describe("Guild ID for guild-scoped settings"),
});

export type SetSettingRequest = z.infer<typeof setSettingRequestSchema>;

/**
 * Set setting response schema (204 No Content)
 */
export const setSettingResponseSchema = z.void();

export type SetSettingResponse = z.infer<typeof setSettingResponseSchema>;

/**
 * Delete setting query parameters
 */
export const deleteSettingQuerySchema = z.object({
    scopeType: scopeTypeSchema.describe("The scope type to delete from"),
    userId: z.string().optional().describe("User ID for user-scoped settings"),
    guildId: z
        .string()
        .optional()
        .describe("Guild ID for guild-scoped settings"),
});

export type DeleteSettingQuery = z.infer<typeof deleteSettingQuerySchema>;

/**
 * Delete setting response schema
 */
export const deleteSettingResponseSchema = z.object({
    deleted: z.boolean().describe("Whether the setting was deleted"),
});

export type DeleteSettingResponse = z.infer<typeof deleteSettingResponseSchema>;

/**
 * List settings query parameters
 */
export const listSettingsQuerySchema = z.object({
    settingsKind: settingsKindSchema.describe("The kind of settings to list"),
    scopeType: scopeTypeSchema.optional().describe("Filter by scope type"),
    userId: z.string().optional().describe("User ID for user settings"),
    guildId: z.string().optional().describe("Guild ID for guild settings"),
});

export type ListSettingsQuery = z.infer<typeof listSettingsQuerySchema>;

/**
 * List settings response schema
 */
export const listSettingsResponseSchema = z.object({
    entries: z.array(settingEntrySchema).describe("List of setting entries"),
});

export type ListSettingsResponse = z.infer<typeof listSettingsResponseSchema>;

/**
 * Setting key ID parameter schema
 */
export const settingKeyIdParamSchema = z.object({
    keyId: z.string().min(1).describe("Setting key identifier"),
});

export type SettingKeyIdParam = z.infer<typeof settingKeyIdParamSchema>;
