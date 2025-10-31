use zako3_hq_server::{
    feature::user::{
        User,
        error::UserError,
        repository::{UpdateUser, UserRepository},
    },
    util::{
        error::{AppError, BusinessError},
        permission::PermissionFlags,
        snowflake::Snowflake,
    },
};

use crate::common::postgres::init_postgres;

pub mod common;

#[tokio::test]
async fn db_user_crud() {
    let db = init_postgres().await;

    let id = Snowflake::new_now();
    let perm = PermissionFlags::all();

    let ident = User {
        id: id.as_lazy(),
        name: "hi".into(),
        permissions: perm.clone(),
    };

    let ident1 = User {
        id: Snowflake::new_now().as_lazy(),
        ..ident.clone()
    };

    {
        db.create_user(ident.clone()).await.unwrap();
        let ident_found = db.find_user(id.as_lazy()).await.unwrap().unwrap();
        assert_eq!(ident_found, ident);
    }

    {
        db.update_user(
            id.as_lazy(),
            UpdateUser {
                permissions: Some(PermissionFlags::empty()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let ident_found = db.find_user(id.as_lazy()).await.unwrap().unwrap();

        assert_eq!(ident_found.name, "hi".to_string());
        assert_eq!(ident_found.permissions, PermissionFlags::empty());
    }

    {
        db.delete_user(id.as_lazy()).await.unwrap();
        let ident_found = db.find_user(id.as_lazy()).await.unwrap();
        assert_eq!(ident_found, None);
    }

    {
        let r = db.delete_user(id.as_lazy()).await;
        assert!(matches!(r, Err(AppError::NotFound)));
    }

    {
        db.create_user(ident.clone()).await.unwrap();
        let r = db.create_user(ident1).await;
        assert!(matches!(
            r,
            Err(AppError::Business(BusinessError::User(
                UserError::DuplicateName
            )))
        ));
    }
}
