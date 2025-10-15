use serde::Serialize;
use utoipa::{
    Modify,
    openapi::{
        OpenApi,
        security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    },
};

#[derive(Debug, Serialize)]
struct SecurityMod;

impl Modify for SecurityMod {
    fn modify(&self, openapi: &mut OpenApi) {
        if let Some(schema) = openapi.components.as_mut() {
            schema.add_security_scheme(
                "admin",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .description(Some("Admin or owned access"))
                        .build(),
                ),
            );

            schema.add_security_scheme(
                "owned",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .description(Some("Owned access"))
                        .build(),
                ),
            );
        }
    }
}

#[derive(utoipa::OpenApi)]
#[openapi(
    info(
        title = "zako3-hq",
        description = "Zako3 Headquarter API",
        contact(name = "MincoMK", email = "mail@drchi.co.kr"),
        version = "v1",
    ),
    modifiers(&SecurityMod),
    security(
        ("admin" = []),
        ("owned" = []),
    )
)]
pub struct ApiDoc;
