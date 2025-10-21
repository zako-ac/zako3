use serde::{Deserialize, Serialize};

use crate::feature::settings::{scope::Field, types::QueuePolicy};

pub mod repository;
pub mod scope;
pub mod service;
pub mod types;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SettingsObject {
    pub queue_policy: scope::Field<QueuePolicy>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub object: SettingsObject,
    pub scope: scope::SettingsScope,
}

/// Merge settings according to priority.
/// The items at the back take higher priority.
///
/// # Returns
/// - `Some(result)` The merged result
/// - `None` if the parameter is empty
pub fn merge_settings(settings_list: &[SettingsObject]) -> Option<SettingsObject> {
    settings_list.iter().cloned().reduce(|a, b| SettingsObject {
        queue_policy: reduce_helper(a.queue_policy, b.queue_policy),
    })
}

fn reduce_helper<T>(a: Field<T>, b: Field<T>) -> Field<T> {
    // a is important => a takes priority
    // a is not important => b takes priority

    if a.important
        && let Some(a_val) = a.value
    {
        Field {
            important: true,
            value: Some(a_val),
        }
    } else {
        Field {
            important: b.important,
            value: b.value.or(a.value),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::feature::settings::{
        SettingsObject, merge_settings, scope::Field, types::QueuePolicy,
    };

    #[test]
    fn test_settings_merge_important() {
        let s1 = SettingsObject {
            queue_policy: Field::new_non_important(QueuePolicy::Off),
        };

        let s2 = SettingsObject {
            queue_policy: Field::new_important(QueuePolicy::User),
        };

        let s3 = SettingsObject {
            queue_policy: Field::new_default(),
        };

        let s4 = SettingsObject {
            queue_policy: Field::new_important(QueuePolicy::TTS),
        };

        let merged = merge_settings(&[s1, s2, s3, s4]).unwrap();
        assert_eq!(merged.queue_policy.value.unwrap(), QueuePolicy::User);
    }

    #[test]
    fn test_settings_merge_non_important() {
        let s1 = SettingsObject {
            queue_policy: Field::new_non_important(QueuePolicy::Off),
        };

        let s2 = SettingsObject {
            queue_policy: Field::new_non_important(QueuePolicy::User),
        };

        let s3 = SettingsObject {
            queue_policy: Field::new_default(),
        };

        let s4 = SettingsObject {
            queue_policy: Field::new_non_important(QueuePolicy::TTS),
        };

        let merged = merge_settings(&[s1, s2, s3, s4]).unwrap();
        assert_eq!(merged.queue_policy.value.unwrap(), QueuePolicy::TTS);
    }
}
