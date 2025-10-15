use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use utoipa::{
    PartialSchema, ToSchema,
    openapi::{KnownFormat, ObjectBuilder, RefOr, Schema, SchemaFormat, Type, schema::SchemaType},
};

bitflags! {
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct PermissionFlags: u32 {
        const BaseUser = 0b00000001;
        const Admin = 0b00000010;
    }
}

impl PartialSchema for PermissionFlags {
    fn schema() -> RefOr<Schema> {
        RefOr::T(Schema::Object(
            ObjectBuilder::new()
                .schema_type(SchemaType::Type(Type::Integer))
                .format(Some(SchemaFormat::KnownFormat(KnownFormat::Int64)))
                .build(),
        ))
    }
}

impl ToSchema for PermissionFlags {}
