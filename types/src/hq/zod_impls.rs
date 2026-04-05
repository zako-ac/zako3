use zod_gen::ZodSchema;

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
