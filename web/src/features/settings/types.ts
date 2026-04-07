export type TextMappingRule = {
    pattern: string
    replacement: string
    case_sensitive: boolean
}

export type EmojiMappingRule = {
    emoji_id: string
    emoji_name: string
    emoji_image_url: string
    replacement: string
}

export type TextReadingRule = 'always' | 'in_voice_channel' | 'on_mic_mute'

export type UserJoinLeaveAlert =
    | { type: 'auto' }
    | { type: 'off' }
    | { type: 'with_different_username'; value: string }
    | { type: 'custom'; value: { join_message: string; leave_message: string } }

export type UserSettings = {
    text_mappings: TextMappingRule[]
    emoji_mappings: EmojiMappingRule[]
    text_reading_rule: TextReadingRule
    user_join_leave_alert: UserJoinLeaveAlert
    max_message_length: number
    enable_tts_queue: boolean
    tts_voice: string | null
}

export const defaultUserSettings: UserSettings = {
    text_mappings: [],
    emoji_mappings: [],
    text_reading_rule: 'always',
    user_join_leave_alert: { type: 'auto' },
    max_message_length: 100,
    enable_tts_queue: true,
    tts_voice: null,
}

export type UserSettingsField<T> =
    | { type: 'none' }
    | { type: 'normal'; value: T }
    | { type: 'important'; value: T }

export type PartialUserSettings = {
    text_mappings: UserSettingsField<TextMappingRule[]>
    emoji_mappings: UserSettingsField<EmojiMappingRule[]>
    text_reading_rule: UserSettingsField<TextReadingRule>
    user_join_leave_alert: UserSettingsField<UserJoinLeaveAlert>
    max_message_length: UserSettingsField<number>
    enable_tts_queue: UserSettingsField<boolean>
    tts_voice: UserSettingsField<string | null>
}

export const emptyPartial: PartialUserSettings = {
    text_mappings: { type: 'none' },
    emoji_mappings: { type: 'none' },
    text_reading_rule: { type: 'none' },
    user_join_leave_alert: { type: 'none' },
    max_message_length: { type: 'none' },
    enable_tts_queue: { type: 'none' },
    tts_voice: { type: 'none' },
}

export function resolvePartial(partial: PartialUserSettings): UserSettings {
    function extract<T>(field: UserSettingsField<T>, def: T): T {
        if (field.type === 'none') return def
        return field.value
    }
    return {
        text_mappings: extract(partial.text_mappings, []),
        emoji_mappings: extract(partial.emoji_mappings, []),
        text_reading_rule: extract(partial.text_reading_rule, 'always'),
        user_join_leave_alert: extract(partial.user_join_leave_alert, { type: 'auto' }),
        max_message_length: extract(partial.max_message_length, 100),
        enable_tts_queue: extract(partial.enable_tts_queue, true),
        tts_voice: extract(partial.tts_voice, null),
    }
}

export function toPartial(settings: UserSettings): PartialUserSettings {
    return {
        text_mappings: { type: 'normal', value: settings.text_mappings },
        emoji_mappings: { type: 'normal', value: settings.emoji_mappings },
        text_reading_rule: { type: 'normal', value: settings.text_reading_rule },
        user_join_leave_alert: { type: 'normal', value: settings.user_join_leave_alert },
        max_message_length: { type: 'normal', value: settings.max_message_length },
        enable_tts_queue: { type: 'normal', value: settings.enable_tts_queue },
        tts_voice: { type: 'normal', value: settings.tts_voice },
    }
}

// Fold two PartialUserSettings layers together (per-field cascade rules).
// `more` is the more-specific scope, `less` is less-specific.
// Used on the frontend to compute upstream for override warnings.
function foldField<T>(
    more: UserSettingsField<T>,
    less: UserSettingsField<T>,
): UserSettingsField<T> {
    if (less.type === 'important') return less
    if (more.type === 'none') return less
    return more
}

export function foldPartial(more: PartialUserSettings, less: PartialUserSettings): PartialUserSettings {
    return {
        text_mappings: foldField(more.text_mappings, less.text_mappings),
        emoji_mappings: foldField(more.emoji_mappings, less.emoji_mappings),
        text_reading_rule: foldField(more.text_reading_rule, less.text_reading_rule),
        user_join_leave_alert: foldField(more.user_join_leave_alert, less.user_join_leave_alert),
        max_message_length: foldField(more.max_message_length, less.max_message_length),
        enable_tts_queue: foldField(more.enable_tts_queue, less.enable_tts_queue),
        tts_voice: foldField(more.tts_voice, less.tts_voice),
    }
}
