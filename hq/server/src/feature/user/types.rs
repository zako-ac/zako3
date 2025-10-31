use regex::Regex;
use serde::Deserialize;
use utoipa::ToSchema;
use validator::{Validate, ValidationError};

use crate::util::permission::PermissionFlags;

#[derive(Clone, Debug, ToSchema, Deserialize, Validate)]
pub struct CreateUser {
    #[validate(length(min = 3, max = 32))]
    #[validate(custom(function = "validate_username"))]
    pub name: String,
    pub permissions: PermissionFlags,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct UpdateUserInfo {
    pub name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct UpdateUserPermissions {
    pub permissions: PermissionFlags,
}

fn validate_username(username: &str) -> Result<(), validator::ValidationError> {
    let regex = Regex::new(r"^[a-z0-9]{3,32}$").unwrap();
    regex.is_match(username).then_some(()).ok_or(
        ValidationError::new("lowercase").with_message("Username must be all lowercase".into()),
    )
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use regex::Regex;

    use crate::feature::user::types::validate_username;

    proptest! {
        #[test]
        fn username_should_lowercase(username in r"[a-zA-Z0-9 ]+") {
            let result = validate_username(&username);
            let expected = Regex::new(r"^[a-z0-9]{3,32}$").unwrap().is_match(&username);

            assert_eq!(result.is_ok(), expected);
        }
    }
}
