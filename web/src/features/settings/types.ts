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
