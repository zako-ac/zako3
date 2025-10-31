use zako3_hq_server::{
    feature::tap::{
        Tap,
        repository::{UpdateTap, TapRepository},
    },
    util::{
        error::AppError,
        snowflake::Snowflake,
    },
};

use crate::common::postgres::init_postgres;

pub mod common;

#[tokio::test]
async fn db_tap_crud() {
    let db = init_postgres().await;

    let id = Snowflake::new_now();

    let tap = Tap {
        id: id.as_lazy(),
        name: "test-tap".to_string().into(),
    };

    let tap1 = Tap {
        id: Snowflake::new_now().as_lazy(),
        ..tap.clone()
    };

    {
        db.create_tap(tap.clone()).await.unwrap();
        let tap_found = db.find_tap(id.as_lazy()).await.unwrap().unwrap();
        let name: String = tap_found.name.clone().into();
        assert_eq!(name, "test-tap");
    }

    {
        db.update_tap(
            id.as_lazy(),
            UpdateTap {
                name: Some("updated-tap".to_string().into()),
            },
        )
        .await
        .unwrap();
        let tap_found = db.find_tap(id.as_lazy()).await.unwrap().unwrap();

        let name: String = tap_found.name.into();
        assert_eq!(name, "updated-tap");
    }

    {
        db.delete_tap(id.as_lazy()).await.unwrap();
        let tap_found = db.find_tap(id.as_lazy()).await.unwrap();
        assert_eq!(tap_found, None);
    }

    {
        let r = db.delete_tap(id.as_lazy()).await;
        assert!(matches!(r, Err(AppError::NotFound)));
    }

    {
        db.create_tap(tap.clone()).await.unwrap();
        let r = db.create_tap(tap1).await;
        assert!(r.is_err());
    }
}
