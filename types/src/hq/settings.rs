use std::collections::HashSet;
use std::hash::Hash;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::TapId;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
pub struct TextMappingRule {
    pub pattern: String,
    pub replacement: String,
    pub case_sensitive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
pub struct EmojiMappingRule {
    pub emoji_id: String,
    pub emoji_name: String,
    pub emoji_image_url: String,
    pub replacement: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextReadingRule {
    Always,
    InVoiceChannel,
    OnMicMute,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub enum UserJoinLeaveAlert {
    Auto,
    Off,

    /// XXX 등장, XXX 퇴장
    WithDifferentUsername(String),

    Custom {
        join_message: String,
        leave_message: String,
    },
}

/// A settings field at a specific scope. `None` means "not configured at this scope".
/// `Important` reverses cascade priority — less-specific scopes with `Important` beat
/// more-specific scopes with `Normal`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum UserSettingsField<T> {
    #[default]
    None,
    Normal(T),
    Important(T),
}

/// Partial (per-scope) settings. Each field is `None` if not configured at this scope.
/// Use `PartialUserSettings::fold` to merge two scopes, and `resolve` to get the
/// final concrete `UserSettings`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
pub struct PartialUserSettings {
    pub text_mappings: UserSettingsField<Vec<TextMappingRule>>,
    pub emoji_mappings: UserSettingsField<Vec<EmojiMappingRule>>,
    pub text_reading_rule: UserSettingsField<TextReadingRule>,
    pub user_join_leave_alert: UserSettingsField<UserJoinLeaveAlert>,
    pub max_message_length: UserSettingsField<u16>,
    pub enable_tts_queue: UserSettingsField<bool>,
    pub tts_voice: UserSettingsField<Option<TapId>>,
}

/// Merge two scalar settings fields.
///
/// Rules:
/// - `less` is `Important(y)` → `Important(y)` (admin override wins regardless)
/// - `more` is `None`         → take `less` as-is
/// - otherwise                → take `more` as-is
fn fold_field<T: Clone>(
    more: &UserSettingsField<T>,
    less: &UserSettingsField<T>,
) -> UserSettingsField<T> {
    match (more, less) {
        (_, UserSettingsField::Important(y)) => UserSettingsField::Important(y.clone()),
        (UserSettingsField::None, less) => less.clone(),
        (more, _) => more.clone(),
    }
}

/// Merge two list settings fields, deduplicating entries by key.
///
/// The Important/Normal/None wrapper determines which side is "primary" (its entries
/// win on key conflicts). Entries from the secondary side whose key is absent in the
/// primary are appended, so no information is silently dropped.
///
/// Priority for choosing primary:
/// 1. `less` is Important  → primary = less  (admin override; important from less-specific scope wins)
/// 2. `more` is None       → primary = less
/// 3. `more` is Important  → primary = more
/// 4. `more` is Normal     → primary = more
fn fold_list_field<T, K>(
    more: &UserSettingsField<Vec<T>>,
    less: &UserSettingsField<Vec<T>>,
    key_fn: impl Fn(&T) -> K,
) -> UserSettingsField<Vec<T>>
where
    T: Clone,
    K: Hash + Eq,
{
    fn entries<T>(f: &UserSettingsField<Vec<T>>) -> &[T] {
        match f {
            UserSettingsField::None => &[],
            UserSettingsField::Normal(v) | UserSettingsField::Important(v) => v.as_slice(),
        }
    }

    let (primary, secondary, wrap_important) = match (more, less) {
        (_, UserSettingsField::Important(_)) => (entries(less), entries(more), true),
        (UserSettingsField::None, _) => (
            entries(less),
            entries(more),
            matches!(less, UserSettingsField::Important(_)),
        ),
        (UserSettingsField::Important(_), _) => (entries(more), entries(less), true),
        (UserSettingsField::Normal(_), _) => (entries(more), entries(less), false),
    };

    if matches!(
        (more, less),
        (UserSettingsField::None, UserSettingsField::None)
    ) {
        return UserSettingsField::None;
    }

    let primary_keys: HashSet<K> = primary.iter().map(&key_fn).collect();
    let mut merged: Vec<T> = primary.to_vec();
    for item in secondary {
        if !primary_keys.contains(&key_fn(item)) {
            merged.push(item.clone());
        }
    }

    if wrap_important {
        UserSettingsField::Important(merged)
    } else {
        UserSettingsField::Normal(merged)
    }
}

impl PartialUserSettings {
    pub fn empty() -> Self {
        Self {
            text_mappings: UserSettingsField::None,
            emoji_mappings: UserSettingsField::None,
            text_reading_rule: UserSettingsField::None,
            user_join_leave_alert: UserSettingsField::None,
            max_message_length: UserSettingsField::None,
            enable_tts_queue: UserSettingsField::None,
            tts_voice: UserSettingsField::None,
        }
    }

    /// Merge two settings layers. More-specific scope goes in `more_important`.
    ///
    /// Per-field rules:
    /// - `less_important` has `Important(y)` → `Important(y)` wins (admin override)
    /// - `more_important` is `None`           → fall through to `less_important`
    /// - otherwise                            → `more_important` wins
    ///
    /// List fields (text_mappings, emoji_mappings) are merged entry-by-entry so
    /// neither scope silently loses unique entries. The primary side's entries win
    /// on key conflicts.
    ///
    /// Full cascade: `fold(guild_user, fold(user, fold(guild, global)))`
    pub fn fold(more: &Self, less: &Self) -> Self {
        Self {
            text_mappings: fold_list_field(&more.text_mappings, &less.text_mappings, |r| {
                (r.pattern.clone(), r.case_sensitive)
            }),
            emoji_mappings: fold_list_field(&more.emoji_mappings, &less.emoji_mappings, |r| {
                r.emoji_id.clone()
            }),
            text_reading_rule: fold_field(&more.text_reading_rule, &less.text_reading_rule),
            user_join_leave_alert: fold_field(
                &more.user_join_leave_alert,
                &less.user_join_leave_alert,
            ),
            max_message_length: fold_field(&more.max_message_length, &less.max_message_length),
            enable_tts_queue: fold_field(&more.enable_tts_queue, &less.enable_tts_queue),
            tts_voice: fold_field(&more.tts_voice, &less.tts_voice),
        }
    }

    /// Apply hardcoded defaults for any remaining `None` fields, producing the
    /// concrete `UserSettings` consumed by TTS/audio.
    pub fn resolve(self) -> UserSettings {
        fn extract<T>(field: UserSettingsField<T>, default: T) -> T {
            match field {
                UserSettingsField::None => default,
                UserSettingsField::Normal(v) | UserSettingsField::Important(v) => v,
            }
        }

        UserSettings {
            text_mappings: extract(self.text_mappings, vec![]),
            emoji_mappings: extract(self.emoji_mappings, vec![]),
            text_reading_rule: extract(self.text_reading_rule, TextReadingRule::Always),
            user_join_leave_alert: extract(self.user_join_leave_alert, UserJoinLeaveAlert::Auto),
            max_message_length: extract(self.max_message_length, 100),
            enable_tts_queue: extract(self.enable_tts_queue, true),
            tts_voice: extract(self.tts_voice, None),
        }
    }
}

/// Resolved (concrete) settings — all fields are present. Produced by
/// `PartialUserSettings::resolve` after cascading all scopes.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
pub struct UserSettings {
    pub text_mappings: Vec<TextMappingRule>,
    pub emoji_mappings: Vec<EmojiMappingRule>,
    pub text_reading_rule: TextReadingRule,
    pub user_join_leave_alert: UserJoinLeaveAlert,
    pub max_message_length: u16,
    pub enable_tts_queue: bool,
    pub tts_voice: Option<TapId>,
}

impl Default for UserSettings {
    fn default() -> Self {
        PartialUserSettings::empty().resolve()
    }
}
