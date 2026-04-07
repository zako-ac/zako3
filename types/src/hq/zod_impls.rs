use zod_gen::ZodSchema;

use super::settings::UserSettingsField;

impl<T: ZodSchema> ZodSchema for UserSettingsField<T> {
    fn zod_schema() -> String {
        let t = T::zod_schema();
        format!(
            r#"z.discriminatedUnion("type", [z.object({{type: z.literal("none")}}), z.object({{type: z.literal("normal"), value: {t}}}), z.object({{type: z.literal("important"), value: {t}}})])"#,
        )
    }
}

macro_rules! impl_zod_schema_for_tuple_struct {
    ($type_name:ty, $inner_type:ty) => {
        impl ZodSchema for $type_name {
            fn zod_schema() -> String {
                <$inner_type as ZodSchema>::zod_schema()
            }
        }
    };
}

impl_zod_schema_for_tuple_struct!(super::user::UserId, u64);
impl_zod_schema_for_tuple_struct!(super::user::DiscordUserId, String);
impl_zod_schema_for_tuple_struct!(super::user::Username, String);
impl_zod_schema_for_tuple_struct!(super::tap::TapId, u64);
impl_zod_schema_for_tuple_struct!(super::tap::TapName, String);
impl_zod_schema_for_tuple_struct!(super::api_key::ApiKeyId, u64);
impl_zod_schema_for_tuple_struct!(super::notification::NotificationId, u64);
