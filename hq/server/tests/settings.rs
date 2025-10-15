use zako3_hq_server::feature::settings::{
    Settings, SettingsObject,
    repository::SettingsRepository,
    scope::{Field, SettingsScope},
    types::QueuePolicy,
};

use crate::common::postgres::init_postgres;

mod common;

#[tokio::test]
async fn test_settings_db() {
    let db = init_postgres().await;

    // insert, update, remove

    let scope = SettingsScope::User(123);

    let mut settings = Settings {
        object: SettingsObject {
            queue_policy: Field::new_important(QueuePolicy::User),
        },
        scope: scope.clone(),
    };

    {
        db.set_settings(&settings).await.unwrap();
        let s_found = db.get_settings(&scope).await.unwrap().unwrap();
        assert_eq!(
            s_found.queue_policy.value,
            settings.object.queue_policy.value
        );
    }

    {
        settings.object.queue_policy.value = Some(QueuePolicy::TTS);

        db.set_settings(&settings).await.unwrap();
        let s_found = db.get_settings(&scope).await.unwrap().unwrap();
        assert_eq!(
            s_found.queue_policy.value,
            settings.object.queue_policy.value
        );
    }

    {
        db.delete_settings(&scope).await.unwrap();
        let s_found = db.get_settings(&scope).await.unwrap();
        assert!(s_found.is_none());
    }
}
