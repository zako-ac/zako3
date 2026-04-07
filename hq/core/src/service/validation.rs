use hq_types::hq::{
    API_KEY_LABEL_MAX_LENGTH, TAP_DESCRIPTION_MAX_LENGTH, TAP_NAME_MAX_LENGTH,
    VERIFICATION_DESCRIPTION_MIN_LENGTH, VERIFICATION_TITLE_MIN_LENGTH,
};

use crate::{CoreError, CoreResult};

pub fn validate_tap_name(name: &str) -> CoreResult<()> {
    if name.is_empty() {
        return Err(CoreError::InvalidInput("Tap name is required".to_string()));
    }
    if name.len() > TAP_NAME_MAX_LENGTH {
        return Err(CoreError::InvalidInput(format!(
            "Tap name must be at most {} characters",
            TAP_NAME_MAX_LENGTH
        )));
    }
    Ok(())
}

pub fn validate_tap_description(description: &Option<String>) -> CoreResult<()> {
    if let Some(desc) = description {
        if desc.len() > TAP_DESCRIPTION_MAX_LENGTH {
            return Err(CoreError::InvalidInput(format!(
                "Description must be at most {} characters",
                TAP_DESCRIPTION_MAX_LENGTH
            )));
        }
    }
    Ok(())
}

pub fn validate_api_key_label(label: &str) -> CoreResult<()> {
    if label.is_empty() {
        return Err(CoreError::InvalidInput(
            "Token label is required".to_string(),
        ));
    }
    if label.len() > API_KEY_LABEL_MAX_LENGTH {
        return Err(CoreError::InvalidInput(format!(
            "Token label must be at most {} characters",
            API_KEY_LABEL_MAX_LENGTH
        )));
    }
    Ok(())
}

pub fn validate_verification_request(title: &str, description: &str) -> CoreResult<()> {
    if title.len() < VERIFICATION_TITLE_MIN_LENGTH {
        return Err(CoreError::InvalidInput(format!(
            "Title must be at least {} characters",
            VERIFICATION_TITLE_MIN_LENGTH
        )));
    }
    if description.len() < VERIFICATION_DESCRIPTION_MIN_LENGTH {
        return Err(CoreError::InvalidInput(format!(
            "Please provide a detailed description (at least {} characters)",
            VERIFICATION_DESCRIPTION_MIN_LENGTH
        )));
    }
    Ok(())
}
